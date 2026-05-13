use anyhow::Result;
use chrono::{DateTime, Utc};
use regex::Regex;
use std::path::{Component, Path, PathBuf};

use super::evidence::{local_verification_task_evidence, GoalReviewEvidence};
use super::state::{
    GoalState, GOAL_AGENT_EXECUTE_TASK_ID, GOAL_ARTIFACTS_DIR, GOAL_CONTROLLER_ACTOR,
    GOAL_LOCAL_VERIFY_TASK_ID, GOAL_REVIEW_ARTIFACTS_DIR, GOAL_REVIEW_FILE, GOAL_REVIEW_TASK_ID,
    GOAL_SECURITY_REVIEW_FILE, GOAL_SECURITY_REVIEW_TASK_ID,
};
use super::task_graph::{goal_task_done, GoalTask, GoalTaskGraph, GoalTaskStatus};
use crate::runtime::events::{
    Event, EventBuilder, EventKind, EventWriter, GateId, RunId, TaskId, WorkerId,
};
use crate::runtime::gates::{gates_passed, GateResult};

pub(crate) fn apply_local_verification_task_result(
    task_graph: &mut GoalTaskGraph,
    gates: &[GateResult],
    completed_at: DateTime<Utc>,
) -> Option<GoalTask> {
    let gates_ok = !gates.is_empty() && gates_passed(gates);
    let task = task_graph
        .tasks
        .iter_mut()
        .find(|task| task.id == GOAL_LOCAL_VERIFY_TASK_ID)?;

    task.status = if gates_ok {
        GoalTaskStatus::Done
    } else {
        GoalTaskStatus::Blocked
    };
    task.owner_role = Some(GOAL_CONTROLLER_ACTOR.to_string());
    task.completed_at = gates_ok.then_some(completed_at);
    task.evidence = local_verification_task_evidence(gates, gates_ok);
    Some(task.clone())
}

pub(crate) async fn append_local_verification_task_events(
    state: &GoalState,
    task: &GoalTask,
) -> Result<()> {
    let writer = EventWriter::new(state.state_dir.join(crate::runtime::config::EVENTS_FILE));
    let builder = EventBuilder::new(RunId(state.goal_id.clone()));
    let worker_id = WorkerId(GOAL_CONTROLLER_ACTOR.to_string());
    let task_id = TaskId(task.id.clone());
    let summary = super::planner::controller_task_summary(task);

    let started = Event::new(RunId(state.goal_id.clone()), EventKind::TaskStarted)
        .with_actor(GOAL_CONTROLLER_ACTOR)
        .with_payload(serde_json::json!({
            "task_id": task.id,
            "worker_id": GOAL_CONTROLLER_ACTOR,
            "title": task.title,
        }))?;

    let finished = if task.status == GoalTaskStatus::Done {
        builder.task_completed(task_id, worker_id, Some(&summary))?
    } else {
        Event::new(RunId(state.goal_id.clone()), EventKind::TaskFailed)
            .with_actor(GOAL_CONTROLLER_ACTOR)
            .with_payload(serde_json::json!({
                "task_id": task.id,
                "worker_id": GOAL_CONTROLLER_ACTOR,
                "summary": summary,
            }))?
    };

    writer.append_many(&[started, finished]).await
}

pub(crate) async fn append_gate_events(state: &GoalState, gates: &[GateResult]) -> Result<()> {
    let writer = EventWriter::new(state.state_dir.join(crate::runtime::config::EVENTS_FILE));
    let builder = EventBuilder::new(RunId(state.goal_id.clone()));
    let mut events = Vec::new();

    for gate in gates {
        let gate_id = GateId(gate.name.clone());
        events.push(builder.command_finished(
            gate_id.clone(),
            &gate.name,
            &gate.command_line,
            gate.exit_code,
            gate.timed_out,
            gate.stdout_summary.as_deref(),
            gate.stderr_summary.as_deref(),
            gate.output_path.as_deref(),
        )?);

        let gate_event = if gate.passed {
            builder.gate_passed_with_evidence(
                gate_id,
                &gate.name,
                gate.required,
                Some(&gate.command_line),
                gate.exit_code,
                gate.timed_out,
                gate.stdout_summary.as_deref(),
                gate.stderr_summary.as_deref(),
                gate.output_path.as_deref(),
                Some(gate.timeout_secs),
            )
        } else {
            builder.gate_failed_with_evidence(
                gate_id,
                &gate.name,
                gate.required,
                Some(&gate.command_line),
                gate.exit_code,
                gate.timed_out,
                gate.stdout_summary.as_deref(),
                gate.stderr_summary.as_deref(),
                gate.output_path.as_deref(),
                Some(gate.timeout_secs),
            )
        }?;
        events.push(gate_event);
    }

    if events.is_empty() {
        return Ok(());
    }
    writer.append_many(&events).await
}

pub(crate) async fn scan_goal_security_findings(
    project_dir: &Path,
    changed_files: &[String],
) -> Result<Vec<String>> {
    let private_key = Regex::new(r"-----BEGIN [A-Z ]*PRIVATE KEY-----")?;
    let secret_assignment =
        Regex::new(r#"(?i)\b(api[_-]?key|secret|token|password)\b\s*[:=]\s*["'][^"']{16,}["']"#)?;
    let mut findings = Vec::new();

    for changed_file in changed_files {
        let Some(path) = safe_project_file_path(project_dir, changed_file) else {
            continue;
        };
        let Ok(metadata) = tokio::fs::metadata(&path).await else {
            continue;
        };
        if !metadata.is_file() || metadata.len() > 512 * 1024 {
            continue;
        }
        let Ok(content) = tokio::fs::read_to_string(&path).await else {
            continue;
        };
        for (line_index, line) in content.lines().enumerate() {
            if private_key.is_match(line) || secret_assignment.is_match(line) {
                findings.push(format!(
                    "{}:{} contains a high-confidence secret marker",
                    changed_file,
                    line_index + 1
                ));
            }
        }
    }

    Ok(findings)
}

fn safe_project_file_path(project_dir: &Path, changed_file: &str) -> Option<PathBuf> {
    let path = Path::new(changed_file);
    if path.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return None;
    }
    Some(project_dir.join(path))
}

pub(crate) async fn write_goal_review_evidence(
    state: &GoalState,
    task_graph: &GoalTaskGraph,
    proof: &super::proof::GoalProof,
    project_dir: &Path,
    generated_at: DateTime<Utc>,
) -> Result<GoalReviewEvidence> {
    let review_dir = PathBuf::from(GOAL_ARTIFACTS_DIR).join(GOAL_REVIEW_ARTIFACTS_DIR);
    let review_path = review_dir.join(GOAL_REVIEW_FILE);
    let security_review_path = review_dir.join(GOAL_SECURITY_REVIEW_FILE);
    let review_abs_dir = state.state_dir.join(&review_dir);
    crate::runtime::config::ensure_private_dir(&review_abs_dir).await?;

    let local_verify_done = goal_task_done(task_graph, GOAL_LOCAL_VERIFY_TASK_ID);
    let agent_execution_done = goal_task_done(task_graph, GOAL_AGENT_EXECUTE_TASK_ID);
    let gates_ok = !proof.gates.is_empty() && gates_passed(&proof.gates);
    let security_findings = scan_goal_security_findings(project_dir, &proof.changed_files).await?;
    let review_artifacts = super::review_artifacts::build_goal_review_artifacts(
        &review_path,
        &security_review_path,
        local_verify_done,
        agent_execution_done,
        &proof.gates,
        &proof.changed_files,
        &security_findings,
    );
    let review_artifact_lines =
        super::review_artifacts::review_artifacts_markdown(&review_artifacts);

    let review_summary = if local_verify_done && agent_execution_done && gates_ok {
        "Controller review passed: local gate evidence and agent execution evidence are present."
            .to_string()
    } else {
        "Controller review blocked: local gates, proof, or agent execution evidence is incomplete."
            .to_string()
    };
    let security_summary = if !agent_execution_done {
        "Security review blocked: agent execution evidence is incomplete.".to_string()
    } else if security_findings.is_empty() {
        "Security review passed: no high-confidence secret markers found in changed files."
            .to_string()
    } else {
        format!(
            "Security review blocked: {} high-confidence secret marker(s) found.",
            security_findings.len()
        )
    };

    let task_lines = task_graph
        .tasks
        .iter()
        .map(|task| format!("- `{}`: `{}`", task.id, task.status))
        .collect::<Vec<_>>()
        .join("\n");
    let gate_lines = if proof.gates.is_empty() {
        "- no gate evidence recorded".to_string()
    } else {
        proof
            .gates
            .iter()
            .map(|gate| {
                let status = if gate.passed { "passed" } else { "failed" };
                format!("- `{}`: `{}`", gate.name, status)
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    let changed_file_lines = if proof.changed_files.is_empty() {
        "- no changed files reported by git diff".to_string()
    } else {
        proof
            .changed_files
            .iter()
            .map(|file| format!("- `{file}`"))
            .collect::<Vec<_>>()
            .join("\n")
    };

    let review_body = format!(
        "# Goal Review\n\n\
         Generated: {generated_at}\n\n\
         Goal ID: `{}`\n\n\
         ## Result\n\n\
         {review_summary}\n\n\
         ## Review Artifacts\n\n\
         {review_artifact_lines}\n\n\
         ## Task Evidence\n\n\
         {task_lines}\n\n\
         ## Gate Evidence\n\n\
         {gate_lines}\n",
        state.goal_id
    );
    crate::runtime::atomic::atomic_write(
        &state.state_dir.join(&review_path),
        review_body.as_bytes(),
    )
    .await?;

    let finding_lines = if security_findings.is_empty() {
        "- no findings".to_string()
    } else {
        security_findings
            .iter()
            .map(|finding| format!("- {finding}"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let security_body = format!(
        "# Goal Security Review\n\n\
         Generated: {generated_at}\n\n\
         Goal ID: `{}`\n\n\
         ## Result\n\n\
         {security_summary}\n\n\
         ## Changed Files Scanned\n\n\
         {changed_file_lines}\n\n\
         ## Findings\n\n\
         {finding_lines}\n",
        state.goal_id
    );
    crate::runtime::atomic::atomic_write(
        &state.state_dir.join(&security_review_path),
        security_body.as_bytes(),
    )
    .await?;

    Ok(GoalReviewEvidence {
        review_path,
        security_review_path,
        review_artifacts,
        security_findings,
    })
}

pub(crate) fn apply_goal_review_task_result(
    task_graph: &mut GoalTaskGraph,
    evidence: &GoalReviewEvidence,
    completed_at: DateTime<Utc>,
) -> Option<GoalTask> {
    let review_ok = goal_task_done(task_graph, GOAL_LOCAL_VERIFY_TASK_ID)
        && goal_task_done(task_graph, GOAL_AGENT_EXECUTE_TASK_ID);
    let task = task_graph
        .tasks
        .iter_mut()
        .find(|task| task.id == GOAL_REVIEW_TASK_ID)?;

    task.status = if review_ok {
        GoalTaskStatus::Done
    } else {
        GoalTaskStatus::Blocked
    };
    task.owner_role = Some(GOAL_CONTROLLER_ACTOR.to_string());
    task.completed_at = review_ok.then_some(completed_at);
    task.evidence = super::review_artifacts::review_task_evidence(evidence);
    Some(task.clone())
}

pub(crate) fn apply_goal_security_review_task_result(
    task_graph: &mut GoalTaskGraph,
    evidence: &GoalReviewEvidence,
    completed_at: DateTime<Utc>,
) -> Option<GoalTask> {
    let security_ok = goal_task_done(task_graph, GOAL_AGENT_EXECUTE_TASK_ID)
        && evidence.security_findings.is_empty();
    let task = task_graph
        .tasks
        .iter_mut()
        .find(|task| task.id == GOAL_SECURITY_REVIEW_TASK_ID)?;

    task.status = if security_ok {
        GoalTaskStatus::Done
    } else {
        GoalTaskStatus::Blocked
    };
    task.owner_role = Some(GOAL_CONTROLLER_ACTOR.to_string());
    task.completed_at = security_ok.then_some(completed_at);
    task.evidence = super::review_artifacts::security_review_task_evidence(evidence);
    Some(task.clone())
}

pub(crate) async fn append_goal_review_task_events(
    state: &GoalState,
    tasks: &[GoalTask],
) -> Result<()> {
    let writer = EventWriter::new(state.state_dir.join(crate::runtime::config::EVENTS_FILE));
    let builder = EventBuilder::new(RunId(state.goal_id.clone()));
    let worker_id = WorkerId(GOAL_CONTROLLER_ACTOR.to_string());
    let mut events = Vec::new();

    for task in tasks {
        let task_id = TaskId(task.id.clone());
        let summary = super::planner::controller_task_summary(task);
        events.push(
            Event::new(RunId(state.goal_id.clone()), EventKind::TaskStarted)
                .with_actor(GOAL_CONTROLLER_ACTOR)
                .with_payload(serde_json::json!({
                    "task_id": task.id,
                    "worker_id": GOAL_CONTROLLER_ACTOR,
                    "title": task.title,
                }))?,
        );
        let finished = if task.status == GoalTaskStatus::Done {
            builder.task_completed(task_id, worker_id.clone(), Some(&summary))?
        } else {
            Event::new(RunId(state.goal_id.clone()), EventKind::TaskFailed)
                .with_actor(GOAL_CONTROLLER_ACTOR)
                .with_payload(serde_json::json!({
                    "task_id": task.id,
                    "worker_id": GOAL_CONTROLLER_ACTOR,
                    "summary": summary,
                }))?
        };
        events.push(finished);
    }

    if events.is_empty() {
        return Ok(());
    }
    writer.append_many(&events).await
}
