use anyhow::{Context, Result};
use serde::Serialize;
use serde_json::Value;
use std::fmt::Write;
use std::path::{Path, PathBuf};

use super::{GoalProof, GoalState, GoalTaskGraph, GOAL_PROOF_FILE};

mod release;
mod sections;

#[derive(Debug, Clone, Serialize)]
pub struct GoalOpenPrDraft {
    pub goal_id: String,
    pub dry_run: bool,
    pub draft: bool,
    pub title: String,
    pub body: String,
    pub head_branch: Option<String>,
    pub existing_pr_url: Option<String>,
    pub proof_path: PathBuf,
}

pub(crate) async fn render_goal_open_pr(goal_id: &str, draft: bool) -> Result<GoalOpenPrDraft> {
    let state = super::resolve_goal(goal_id).await?;
    let proof_path = state.state_dir.join(GOAL_PROOF_FILE);
    if !tokio::fs::try_exists(&proof_path).await? {
        return missing_evidence(&state, goal_id, "goal proof is missing");
    }

    let proof = GoalProof::load(&state.state_dir)
        .await
        .with_context(|| format!("Failed to load goal proof for {}", state.goal_id))?;
    let task_graph = GoalTaskGraph::load(&state.state_dir)
        .await
        .with_context(|| format!("Failed to load goal task graph for {}", state.goal_id))?;
    let delivery_metadata = super::task_graph::load_task_delivery_metadata(&state.state_dir)
        .await
        .with_context(|| {
            format!(
                "Failed to load goal delivery metadata for {}",
                state.goal_id
            )
        })?;

    if !has_pr_evidence(&proof, &delivery_metadata) {
        return missing_evidence(&state, goal_id, "proof evidence is missing");
    }

    let head_branch = select_head_branch(&proof, &delivery_metadata);
    let existing_pr_url = select_pr_url(&delivery_metadata);
    let title = format!("Goal proof: {}", state.normalized_goal);
    let body = render_pr_body(&state, &proof, &task_graph, &delivery_metadata, draft);
    Ok(GoalOpenPrDraft {
        goal_id: state.goal_id,
        dry_run: true,
        draft,
        title,
        body,
        head_branch,
        existing_pr_url,
        proof_path,
    })
}

fn missing_evidence(
    state: &GoalState,
    requested_goal_id: &str,
    reason: &str,
) -> Result<GoalOpenPrDraft> {
    let next_goal_id = if requested_goal_id == "latest" {
        "latest"
    } else {
        state.goal_id.as_str()
    };
    anyhow::bail!(
        "Goal '{}' {reason}.\nNext: omk goal execute {next_goal_id}",
        state.goal_id
    );
}

fn has_pr_evidence(proof: &GoalProof, delivery_metadata: &[Value]) -> bool {
    !proof.gates.is_empty()
        || !proof.changed_files.is_empty()
        || !delivery_metadata.is_empty()
        || proof.artifacts.iter().any(|artifact| {
            matches!(
                artifact.kind.as_str(),
                "agent_run" | "review" | "security_review"
            )
        })
}

fn render_pr_body(
    state: &GoalState,
    proof: &GoalProof,
    task_graph: &GoalTaskGraph,
    delivery_metadata: &[Value],
    draft: bool,
) -> String {
    let mut body = String::new();
    let proof_value = serde_json::to_value(proof).unwrap_or(Value::Null);
    push_goal_section(&mut body, state, proof, draft);
    push_task_summary(&mut body, proof, task_graph);
    push_delivery_metadata(&mut body, delivery_metadata);
    push_proof_summary(&mut body, proof);
    push_verification_wall(&mut body, proof);
    release::push_release_candidate_notes(&mut body, state, proof, draft);
    sections::push_review_evidence(&mut body, &proof_value);
    sections::push_integration_evidence(&mut body, &proof_value);
    sections::push_oracle_evidence(&mut body, &proof_value);
    push_known_gaps(&mut body, proof);
    push_changed_files(&mut body, proof);
    push_artifacts(&mut body, proof);
    body
}

fn push_goal_section(body: &mut String, state: &GoalState, proof: &GoalProof, draft: bool) {
    push_heading(body, "Goal");
    push_line(body, &format!("- Goal id: `{}`", state.goal_id));
    push_line(body, &format!("- Objective: {}", state.original_goal));
    push_line(body, &format!("- Status: `{}`", proof.status));
    push_line(body, &format!("- Readiness: {}", proof.readiness));
    push_line(body, &format!("- Draft: `{draft}`"));
    push_line(
        body,
        &format!(
            "- Proof path: `{}`",
            state.state_dir.join(GOAL_PROOF_FILE).display()
        ),
    );
    push_blank(body);
}

fn push_task_summary(body: &mut String, proof: &GoalProof, task_graph: &GoalTaskGraph) {
    push_heading(body, "Task Summary");
    push_line(
        body,
        &format!("- Total: {}", proof.task_graph_summary.total_tasks),
    );
    push_line(
        body,
        &format!("- Done: {}", proof.task_graph_summary.done_tasks),
    );
    push_line(
        body,
        &format!("- Pending: {}", proof.task_graph_summary.pending_tasks),
    );
    push_line(
        body,
        &format!("- Blocked: {}", proof.task_graph_summary.blocked_tasks),
    );
    for task in &task_graph.tasks {
        let owner = task
            .owner_role
            .as_deref()
            .map(|owner| format!(" owner={owner}"))
            .unwrap_or_default();
        push_line(
            body,
            &format!("- `{}` `{}`{}: {}", task.id, task.status, owner, task.title),
        );
    }
    push_blank(body);
}

fn push_delivery_metadata(body: &mut String, delivery_metadata: &[Value]) {
    push_heading(body, "Delivery Metadata");
    if delivery_metadata.is_empty() {
        push_line(body, "- No owner/write-scope delivery metadata recorded.");
        push_blank(body);
        return;
    }

    for delivery in delivery_metadata {
        if let Some(task_id) = value_str(delivery, "task_id") {
            push_line(body, &format!("- task: `{task_id}`"));
        } else {
            push_line(body, "- task: `unknown`");
        }
        if let Some(slice_id) = value_str(delivery, "slice_id") {
            push_line(body, &format!("  - slice: {slice_id}"));
        }
        if let Some(owner) = value_str(delivery, "owner") {
            push_line(body, &format!("  - owner: {owner}"));
        }
        if let Some(branch) = value_str(delivery, "branch") {
            push_line(body, &format!("  - branch: {branch}"));
        }
        if let Some(worktree) = value_str(delivery, "worktree_path") {
            push_line(body, &format!("  - worktree: {worktree}"));
        }
        if let Some(pr_link) =
            value_str(delivery, "pr_url").or_else(|| value_str(delivery, "pr_link"))
        {
            push_line(body, &format!("  - pr: {pr_link}"));
        }
        if let Some(summary) = value_str(delivery, "verification_summary") {
            push_line(body, &format!("  - verification: {summary}"));
        }
        if let Some(write_scope) = string_array(delivery.get("write_scope")) {
            push_line(body, "  - write scope:");
            for path in write_scope {
                push_line(body, &format!("    - `{path}`"));
            }
        }
    }
    push_blank(body);
}

fn push_proof_summary(body: &mut String, proof: &GoalProof) {
    push_heading(body, "Proof Summary");
    push_line(body, &format!("- Summary: {}", proof.summary));
    push_line(body, &format!("- Generated: `{}`", proof.generated_at));
    if let Some(git) = &proof.git {
        push_line(body, &format!("- Branch: `{}`", git.branch));
        push_line(body, &format!("- Head: `{}`", git.head));
        push_line(body, &format!("- Dirty: `{}`", git.dirty));
    }
    if proof.commits.is_empty() {
        push_line(body, "- Commits: none recorded");
    } else {
        push_line(body, "- Commits:");
        for commit in &proof.commits {
            push_line(body, &format!("  - `{commit}`"));
        }
    }
    push_blank(body);
}

fn push_verification_wall(body: &mut String, proof: &GoalProof) {
    push_heading(body, "Verification Wall");
    if proof.gates.is_empty() {
        push_line(body, "- No verification gates recorded.");
    } else {
        for gate in &proof.gates {
            let status = if gate.passed { "passed" } else { "failed" };
            let exit_code = gate
                .exit_code
                .map(|code| code.to_string())
                .unwrap_or_else(|| "unknown".to_string());
            push_line(
                body,
                &format!(
                    "- `{}`: {} required={} exit={} duration={}ms",
                    gate.name, status, gate.required, exit_code, gate.duration_ms
                ),
            );
            if let Some(path) = gate.output_path.as_deref() {
                push_line(body, &format!("  - artifact: `{path}`"));
            }
        }
    }
    push_line(
        body,
        &format!(
            "- Post-mutation gates ran: `{}`",
            proof.post_mutation_gates_ran
        ),
    );
    push_blank(body);
}

fn push_known_gaps(body: &mut String, proof: &GoalProof) {
    push_heading(body, "Known Gaps");
    if proof.known_gaps.is_empty() {
        push_line(body, "- None recorded.");
    } else {
        for gap in &proof.known_gaps {
            push_line(body, &format!("- {gap}"));
        }
    }
    if !proof.human_decisions_required.is_empty() {
        push_line(body, "- Human decisions required:");
        for decision in &proof.human_decisions_required {
            push_line(body, &format!("  - {decision}"));
        }
    }
    push_blank(body);
}

fn push_changed_files(body: &mut String, proof: &GoalProof) {
    push_heading(body, "Changed Files");
    if proof.changed_files.is_empty() {
        push_line(body, "- None recorded.");
    } else {
        let mut changed_files = proof.changed_files.clone();
        changed_files.sort();
        for path in changed_files {
            push_line(body, &format!("- `{path}`"));
        }
    }
    push_blank(body);
}

fn push_artifacts(body: &mut String, proof: &GoalProof) {
    push_heading(body, "Artifacts");
    if proof.artifacts.is_empty() {
        push_line(body, "- None recorded.");
    } else {
        for artifact in &proof.artifacts {
            push_line(
                body,
                &format!("- `{}`: `{}`", artifact.kind, display_path(&artifact.path)),
            );
        }
    }
}

fn display_path(path: &Path) -> String {
    path.display().to_string()
}

fn select_head_branch(proof: &GoalProof, delivery_metadata: &[Value]) -> Option<String> {
    delivery_metadata
        .iter()
        .find_map(|delivery| value_str(delivery, "branch").map(str::to_string))
        .or_else(|| {
            proof
                .git
                .as_ref()
                .map(|git| git.branch.clone())
                .filter(|branch| !branch.trim().is_empty())
        })
}

fn select_pr_url(delivery_metadata: &[Value]) -> Option<String> {
    delivery_metadata.iter().find_map(|delivery| {
        value_str(delivery, "pr_url")
            .or_else(|| value_str(delivery, "pr_link"))
            .map(str::to_string)
    })
}

pub(super) fn push_heading(body: &mut String, heading: &str) {
    let _ = writeln!(body, "## {heading}");
}

pub(super) fn push_line(body: &mut String, line: &str) {
    let _ = writeln!(body, "{line}");
}

pub(super) fn push_blank(body: &mut String) {
    body.push('\n');
}

pub(super) fn value_str<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

pub(super) fn string_array(value: Option<&Value>) -> Option<Vec<String>> {
    let values = value?.as_array()?;
    Some(
        values
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect(),
    )
    .filter(|values: &Vec<String>| !values.is_empty())
}
