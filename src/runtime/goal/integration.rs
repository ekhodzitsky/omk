use anyhow::Result;
use chrono::Utc;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

use super::proof::{carry_goal_proof_sidecars, write_json_artifact, GoalProof};
use super::state::{
    FileSystemGoalStateStore, GoalFailure, GoalPhase, GoalState, GoalStateStore, GoalStatus,
    GOAL_ARTIFACTS_DIR, GOAL_LOCAL_VERIFY_TASK_ID, GOAL_PROOF_FILE,
    GOAL_REVIEW_TASK_ID, GOAL_SECURITY_REVIEW_TASK_ID,
};
use super::task_graph::{goal_agent_execution_done, goal_task_done, GoalTaskGraph};
use crate::runtime::events::{EventBuilder, EventWriter, RunId};
use crate::runtime::gates::gates_passed;

const GOAL_INTEGRATION_ARTIFACTS_DIR: &str = "integration";
const GOAL_INTEGRATION_ACCEPT_FILE: &str = "integrator-accept.json";
const GOAL_INTEGRATION_REJECT_FILE: &str = "integrator-reject.json";
const GOAL_INTEGRATION_ROLLBACK_FILE: &str = "rollback-rejected-slice.md";

pub(crate) async fn accept_goal(
    goal_id: &str,
    summary: &str,
    project_dir: &Path,
) -> Result<GoalProof> {
    let mut state = super::resolve_goal(goal_id).await?;
    ensure_integrator_can_decide(&state)?;
    let task_graph = GoalTaskGraph::load(&state.state_dir).await?;
    let proof = GoalProof::load(&state.state_dir).await?;
    let oracle =
        super::oracle::assess_goal_oracle_evidence(&state.normalized_goal, &oracle_gates(&proof));
    let oracle_evidence = super::oracle::oracle_evidence_json(&oracle);
    let mut missing = missing_ready_evidence(&task_graph, &proof);
    if !oracle.passed {
        missing.push(format!(
            "{} oracle evidence is incomplete or failing",
            oracle.kind.as_str()
        ));
    }

    let now = Utc::now();
    let integration_evidence = integration_evidence_json(
        "accepted",
        summary,
        now,
        missing.clone(),
        artifact_path(GOAL_INTEGRATION_ACCEPT_FILE),
    );
    ensure_integration_dir(&state).await?;
    write_json_artifact(
        &state
            .state_dir
            .join(artifact_path(GOAL_INTEGRATION_ACCEPT_FILE)),
        &integration_evidence,
    )
    .await?;

    super::evidence::record_artifact_path_once(
        &mut state,
        "integration_acceptance",
        artifact_path(GOAL_INTEGRATION_ACCEPT_FILE),
        now,
    );
    let mut accepted_proof = proof.clone();
    accepted_proof.generated_at = now;
    accepted_proof.artifacts = state.artifacts.clone();
    accepted_proof.known_gaps = missing.clone();
    if missing.is_empty() {
        state.status = GoalStatus::Ready;
        state.completed_at = Some(now);
        accepted_proof.status = GoalStatus::Ready;
        accepted_proof.readiness = "ready: integration and oracle evidence passed".to_string();
        accepted_proof.summary = format!(
            "Goal '{}' is proof-backed ready: gates, execution, review, integration, and oracle evidence passed.",
            state.normalized_goal
        );
    } else {
        state.status = GoalStatus::NotReady;
        state.completed_at = Some(now);
        accepted_proof.status = GoalStatus::NotReady;
        accepted_proof.readiness =
            "not ready: integrator acceptance found missing proof evidence".to_string();
        accepted_proof.summary = format!(
            "Goal '{}' cannot be accepted as ready until missing evidence is resolved.",
            state.normalized_goal
        );
    }
    finish_integrator_decision(
        state,
        proof,
        accepted_proof,
        integration_evidence,
        oracle_evidence,
        now,
        project_dir,
    )
    .await
}

pub(crate) async fn reject_goal(
    goal_id: &str,
    reason: &str,
    project_dir: &Path,
) -> Result<GoalProof> {
    let mut state = super::resolve_goal(goal_id).await?;
    ensure_integrator_can_decide(&state)?;
    let proof = GoalProof::load(&state.state_dir).await?;
    let oracle =
        super::oracle::assess_goal_oracle_evidence(&state.normalized_goal, &oracle_gates(&proof));
    let oracle_evidence = super::oracle::oracle_evidence_json(&oracle);
    let now = Utc::now();
    let integration_evidence = integration_evidence_json(
        "rejected",
        reason,
        now,
        vec![reason.to_string()],
        artifact_path(GOAL_INTEGRATION_REJECT_FILE),
    );
    ensure_integration_dir(&state).await?;
    write_json_artifact(
        &state
            .state_dir
            .join(artifact_path(GOAL_INTEGRATION_REJECT_FILE)),
        &integration_evidence,
    )
    .await?;

    state.status = GoalStatus::NotReady;
    state.completed_at = Some(now);
    state.failure = Some(GoalFailure {
        reason: reason.to_string(),
        recorded_at: now,
    });
    super::evidence::record_artifact_path_once(
        &mut state,
        "integration_rejection",
        artifact_path(GOAL_INTEGRATION_REJECT_FILE),
        now,
    );
    write_rejection_rollback_plan(&state, &proof, reason).await?;
    super::evidence::record_artifact_path_once(
        &mut state,
        "integration_rollback_plan",
        artifact_path(GOAL_INTEGRATION_ROLLBACK_FILE),
        now,
    );

    let mut rejected_proof = proof.clone();
    rejected_proof.status = GoalStatus::NotReady;
    rejected_proof.readiness = "not ready: integration rejected by local integrator".to_string();
    rejected_proof.summary = format!(
        "Goal '{}' was rejected by the local integrator: {reason}",
        state.normalized_goal
    );
    rejected_proof.generated_at = now;
    rejected_proof.artifacts = state.artifacts.clone();
    rejected_proof.known_gaps = vec![reason.to_string()];
    finish_integrator_decision(
        state,
        proof,
        rejected_proof,
        integration_evidence,
        oracle_evidence,
        now,
        project_dir,
    )
    .await
}

fn ensure_integrator_can_decide(state: &GoalState) -> Result<()> {
    match state.status {
        GoalStatus::Paused => anyhow::bail!(
            "Goal '{}' is paused; run `omk goal resume {}` before integrator review",
            state.goal_id,
            state.goal_id
        ),
        GoalStatus::BlockedOnHuman => anyhow::bail!(
            "Goal '{}' is blocked_on_human and needs testable success criteria first",
            state.goal_id
        ),
        GoalStatus::Cancelled => anyhow::bail!("Goal '{}' is cancelled", state.goal_id),
        _ => Ok(()),
    }
}

fn missing_ready_evidence(task_graph: &GoalTaskGraph, proof: &GoalProof) -> Vec<String> {
    let mut missing = Vec::new();
    if proof.gates.is_empty() || !gates_passed(&proof.gates) {
        missing.push("required verification gates are missing or failing".to_string());
    }
    if !goal_task_done(task_graph, GOAL_LOCAL_VERIFY_TASK_ID) {
        missing.push("local verification task evidence is missing".to_string());
    }
    if !goal_agent_execution_done(task_graph) {
        missing.push("agent execution task evidence is missing".to_string());
    }
    if !goal_task_done(task_graph, GOAL_REVIEW_TASK_ID) {
        missing.push("review task evidence is missing".to_string());
    }
    if !goal_task_done(task_graph, GOAL_SECURITY_REVIEW_TASK_ID) {
        missing.push("security review task evidence is missing".to_string());
    }
    if proof.changed_files.is_empty() {
        missing.push("changed-file evidence is missing".to_string());
    }
    if !proof.post_mutation_gates_ran {
        missing.push("post-mutation verification evidence is missing".to_string());
    }
    let review_artifacts = super::proof::collect_review_artifacts(
        goal_task_done(task_graph, GOAL_REVIEW_TASK_ID),
        goal_task_done(task_graph, GOAL_SECURITY_REVIEW_TASK_ID),
        &proof.gates,
        &proof.changed_files,
    );
    if !review_artifacts_passed(&review_artifacts) {
        missing.push("required review wall evidence is incomplete or blocked".to_string());
    }
    missing
}

fn review_artifacts_passed(artifacts: &[Value]) -> bool {
    !artifacts.is_empty()
        && artifacts.iter().all(|artifact| {
            artifact
                .get("status")
                .and_then(Value::as_str)
                .is_some_and(|status| status == "passed")
        })
}

fn oracle_gates(proof: &GoalProof) -> Vec<super::oracle::GoalOracleGate> {
    proof
        .gates
        .iter()
        .map(|gate| super::oracle::GoalOracleGate {
            name: gate.name.clone(),
            passed: gate.passed,
        })
        .collect()
}

async fn finish_integrator_decision(
    mut state: GoalState,
    prior_proof: GoalProof,
    mut proof: GoalProof,
    integration_evidence: Value,
    oracle_evidence: Value,
    now: chrono::DateTime<Utc>,
    project_dir: &Path,
) -> Result<GoalProof> {
    state.phase = GoalPhase::Proof;
    state.updated_at = now;
    FileSystemGoalStateStore::new().save(&state).await?;

    proof.git = super::evidence::detect_git_evidence(project_dir)
        .await
        .or(proof.git);
    carry_goal_proof_sidecars(&prior_proof, &proof, integration_evidence, oracle_evidence);
    write_json_artifact(&state.state_dir.join(GOAL_PROOF_FILE), &proof).await?;
    append_proof_event(&state, &proof).await?;
    super::budget::append_budget_checkpoint(&state, "integration_decision").await?;
    Ok(proof)
}

async fn append_proof_event(state: &GoalState, proof: &GoalProof) -> Result<()> {
    let writer = EventWriter::new(state.state_dir.join(crate::runtime::config::EVENTS_FILE));
    let builder = EventBuilder::new(RunId(state.goal_id.clone()));
    writer
        .append(&builder.proof_written(
            &state.state_dir.join(GOAL_PROOF_FILE),
            &proof.status.to_string(),
        )?)
        .await
}

fn integration_evidence_json(
    status: &str,
    summary: &str,
    decided_at: chrono::DateTime<Utc>,
    missing_evidence: Vec<String>,
    artifact_path: PathBuf,
) -> Value {
    json!({
        "status": status,
        "summary": summary,
        "decided_at": decided_at,
        "artifact_path": artifact_path,
        "missing_evidence": missing_evidence,
    })
}

fn artifact_path(file_name: &str) -> PathBuf {
    PathBuf::from(GOAL_ARTIFACTS_DIR)
        .join(GOAL_INTEGRATION_ARTIFACTS_DIR)
        .join(file_name)
}

async fn write_rejection_rollback_plan(
    state: &GoalState,
    proof: &GoalProof,
    reason: &str,
) -> Result<()> {
    let changed_files = rollback_changed_files(proof);
    let body = format!(
        "# Rejected Goal Rollback Plan\n\n\
         Goal: `{}`\n\n\
         Rejection reason: {}\n\n\
         ## Scope\n\n\
         Review and revert only the files changed by this rejected goal slice:\n\n\
         {}\
         ## Recovery Steps\n\n\
         1. Keep the durable goal state and proof artifacts for auditability.\n\
         2. Revert or replace the rejected code changes in a task-scoped branch.\n\
         3. Re-run the goal verification gates and integrator review before accepting again.\n",
        state.goal_id, reason, changed_files
    );
    crate::runtime::atomic::atomic_write(
        &state
            .state_dir
            .join(artifact_path(GOAL_INTEGRATION_ROLLBACK_FILE)),
        body.as_bytes(),
    )
    .await
}

fn rollback_changed_files(proof: &GoalProof) -> String {
    if proof.changed_files.is_empty() {
        return "- no changed-file evidence recorded\n".to_string();
    }

    proof
        .changed_files
        .iter()
        .map(|file| format!("- `{file}`\n"))
        .collect()
}

async fn ensure_integration_dir(state: &GoalState) -> Result<()> {
    crate::runtime::config::ensure_private_dir(
        &state
            .state_dir
            .join(GOAL_ARTIFACTS_DIR)
            .join(GOAL_INTEGRATION_ARTIFACTS_DIR),
    )
    .await
}
