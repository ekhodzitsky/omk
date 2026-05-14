use anyhow::Result;
use chrono::{DateTime, Utc};
use regex::Regex;
use serde_json::Value;
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

    // Canonicalize the project root once. When the path cannot be
    // canonicalized (the scanner can be exercised against ephemeral or
    // synthetic paths in tests) the per-file canonicalize step below also
    // fails, so the containment check is enforced consistently.
    let canonical_project_dir = tokio::fs::canonicalize(project_dir).await.ok();

    for changed_file in changed_files {
        let Some(path) = safe_project_file_path(project_dir, changed_file) else {
            continue;
        };
        // Resolve symlinks against the canonical project root before
        // reading. A changed file that escapes the project tree — for
        // example via a symlink planted by an upstream merge — must not be
        // scanned, both to avoid information disclosure through the
        // security review artifact and to keep findings traceable to repo
        // paths the reviewer can audit.
        let resolved = match tokio::fs::canonicalize(&path).await {
            Ok(canon) => canon,
            Err(_) => continue,
        };
        if let Some(root) = &canonical_project_dir {
            if !resolved.starts_with(root) {
                continue;
            }
        }
        let Ok(metadata) = tokio::fs::metadata(&resolved).await else {
            continue;
        };
        if !metadata.is_file() || metadata.len() > 512 * 1024 {
            continue;
        }
        let Ok(content) = tokio::fs::read_to_string(&resolved).await else {
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

fn review_wall_markdown(artifacts: &[Value]) -> String {
    if artifacts.is_empty() {
        return "No structured review wall artifacts were produced.".to_string();
    }

    artifacts
        .iter()
        .map(review_artifact_markdown)
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn review_artifact_markdown(artifact: &Value) -> String {
    let pass = artifact
        .get("pass")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let status = artifact
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let recommended_next_step = artifact
        .get("recommended_next_step")
        .and_then(Value::as_str)
        .unwrap_or("No next step recorded.");
    format!(
        "## {}\n\n\
         Status: `{status}`\n\n\
         Evidence:\n{}\n\n\
         Risks:\n{}\n\n\
         Known gaps:\n{}\n\n\
         Recommended next step: {recommended_next_step}",
        review_pass_title(pass),
        markdown_list(artifact, "evidence"),
        markdown_list(artifact, "risks"),
        markdown_list(artifact, "known_gaps"),
    )
}

fn review_pass_title(pass: &str) -> &'static str {
    match pass {
        "architect" => "Architect",
        "code" => "Code",
        "test" => "Test",
        "security" => "Security",
        "performance" => "Performance",
        "anti-slop" => "Anti-Slop",
        _ => "Unknown",
    }
}

fn markdown_list(artifact: &Value, field: &str) -> String {
    let values = artifact
        .get(field)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>();
    if values.is_empty() {
        return "- none".to_string();
    }
    values
        .iter()
        .map(|value| format!("- {value}"))
        .collect::<Vec<_>>()
        .join("\n")
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
    let review_ok = local_verify_done && agent_execution_done;
    let security_ok = agent_execution_done && security_findings.is_empty();
    let review_artifacts = super::proof::collect_review_artifacts(
        review_ok,
        security_ok,
        &proof.gates,
        &proof.changed_files,
    );
    let review_wall = review_wall_markdown(&review_artifacts);

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
         ## Review Wall\n\n\
         {review_wall}\n\n\
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
        review_summary,
        security_summary,
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
    task.evidence = vec![super::task_graph::GoalTaskEvidence {
        kind: "review".to_string(),
        path: evidence.review_path.clone(),
        summary: evidence.review_summary.clone(),
    }];
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
    task.evidence = vec![super::task_graph::GoalTaskEvidence {
        kind: "security_review".to_string(),
        path: evidence.security_review_path.clone(),
        summary: evidence.security_summary.clone(),
    }];
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn safe_project_file_path_rejects_absolute_paths() {
        let root = Path::new("/tmp/omk-verifier-tests-nonexistent");
        assert!(safe_project_file_path(root, "/etc/passwd").is_none());
        assert!(safe_project_file_path(root, "../escape.txt").is_none());
    }

    #[test]
    fn safe_project_file_path_joins_relative_paths() {
        let root = Path::new("/tmp/omk-verifier-tests-nonexistent");
        let path = safe_project_file_path(root, "src/lib.rs").unwrap();
        assert_eq!(path, root.join("src/lib.rs"));
    }

    #[tokio::test]
    async fn scan_finds_inline_secret_assignment() {
        let dir = tempdir().unwrap();
        let project = dir.path();
        tokio::fs::write(
            project.join("creds.txt"),
            "api_key = \"AAAAAAAAAAAAAAAA-leaked-token-1\"\n",
        )
        .await
        .unwrap();

        let findings = scan_goal_security_findings(project, &["creds.txt".to_string()])
            .await
            .unwrap();

        assert_eq!(findings.len(), 1);
        assert!(findings[0].starts_with("creds.txt:1"));
    }

    #[tokio::test]
    async fn scan_skips_paths_that_escape_project_root_via_symlink() {
        // Plant a real secret outside the project root, then symlink to it
        // from inside the project. The scanner must canonicalize first and
        // refuse to read content that lives outside the project tree —
        // otherwise it would silently report a finding for `internal.rs`
        // that actually came from a file the reviewer cannot reach.
        let outside_dir = tempdir().unwrap();
        let outside_secret = outside_dir.path().join("stolen.txt");
        tokio::fs::write(
            &outside_secret,
            "api_key = \"BBBBBBBBBBBBBBBB-stolen-token-2\"\n",
        )
        .await
        .unwrap();

        let project_dir = tempdir().unwrap();
        let project = project_dir.path();
        let inside_link = project.join("internal.rs");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&outside_secret, &inside_link).unwrap();
        #[cfg(not(unix))]
        {
            // Symlinks on non-Unix targets require elevated privileges; we
            // only need the Unix behaviour to be locked down here.
            let _ = &inside_link;
            return;
        }

        let findings = scan_goal_security_findings(project, &["internal.rs".to_string()])
            .await
            .unwrap();

        assert!(
            findings.is_empty(),
            "scanner must refuse symlinked files that escape the project root; got {findings:?}",
        );
    }

    #[tokio::test]
    async fn scan_follows_symlink_that_stays_inside_project_root() {
        // The defense is "must stay inside the project tree", not "no
        // symlinks at all". A symlink whose target remains under the
        // project root is benign and the scanner should still inspect it.
        let project_dir = tempdir().unwrap();
        let project = project_dir.path();
        let real = project.join("real.txt");
        tokio::fs::write(&real, "password = \"CCCCCCCCCCCCCCCC-internal-secret-3\"\n")
            .await
            .unwrap();
        let link = project.join("alias.txt");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&real, &link).unwrap();
        #[cfg(not(unix))]
        {
            let _ = &link;
            return;
        }

        let findings = scan_goal_security_findings(project, &["alias.txt".to_string()])
            .await
            .unwrap();

        assert_eq!(findings.len(), 1);
        assert!(findings[0].starts_with("alias.txt:1"));
    }
}
