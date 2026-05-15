use anyhow::Result;
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::path::{Path, PathBuf};

use super::super::evidence::GoalReviewEvidence;
use super::super::state::{
    GoalState, GOAL_ARTIFACTS_DIR, GOAL_REVIEW_ARTIFACTS_DIR, GOAL_REVIEW_FILE,
    GOAL_SECURITY_REVIEW_FILE,
};
use super::super::task_graph::{goal_task_done, GoalTaskGraph};
use crate::runtime::gates::gates_passed;

pub(crate) fn review_wall_markdown(artifacts: &[Value]) -> String {
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
    proof: &super::super::proof::GoalProof,
    project_dir: &Path,
    generated_at: DateTime<Utc>,
) -> Result<GoalReviewEvidence> {
    let review_dir = PathBuf::from(GOAL_ARTIFACTS_DIR).join(GOAL_REVIEW_ARTIFACTS_DIR);
    let review_path = review_dir.join(GOAL_REVIEW_FILE);
    let security_review_path = review_dir.join(GOAL_SECURITY_REVIEW_FILE);
    let review_abs_dir = state.state_dir.join(&review_dir);
    crate::runtime::config::ensure_private_dir(&review_abs_dir).await?;

    let local_verify_done = goal_task_done(task_graph, super::super::state::GOAL_LOCAL_VERIFY_TASK_ID);
    let agent_execution_done = goal_task_done(task_graph, super::super::state::GOAL_AGENT_EXECUTE_TASK_ID);
    let gates_ok = !proof.gates.is_empty() && gates_passed(&proof.gates);
    let security_findings =
        super::scan_goal_security_findings(project_dir, &proof.changed_files).await?;
    let review_ok = local_verify_done && agent_execution_done;
    let security_ok = agent_execution_done && security_findings.is_empty();
    let review_artifacts = super::super::proof::collect_review_artifacts(
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
