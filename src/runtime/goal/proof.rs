use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize, Serializer};
use serde_json::Value;
use std::path::Path;

use super::state::{
    GoalState, GoalStatus, GOAL_PROOF_FILE, GOAL_REVIEW_TASK_ID,
    GOAL_SECURITY_REVIEW_TASK_ID,
};
use super::task_graph::{
    goal_agent_execution_done, summarize_task_graph, GoalTaskGraph, GoalTaskGraphSummary,
    GoalTaskStatus,
};
use crate::runtime::gates::{gates_passed, GateResult};

mod artifact;
mod review;
mod sidecar;
mod status;

pub(crate) use artifact::write_json_artifact;
pub(crate) use review::collect_review_artifacts;

#[derive(Debug, Clone, Deserialize)]
pub struct GoalProof {
    pub version: u32,
    pub goal_id: String,
    pub status: GoalStatus,
    pub readiness: String,
    pub summary: String,
    pub generated_at: DateTime<Utc>,
    pub artifacts: Vec<super::state::GoalArtifact>,
    pub task_graph_summary: GoalTaskGraphSummary,
    pub changed_files: Vec<String>,
    pub commits: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub git: Option<super::evidence::GoalGitEvidence>,
    pub gates: Vec<GateResult>,
    #[serde(default)]
    pub post_mutation_gates_ran: bool,
    pub known_gaps: Vec<String>,
    pub human_decisions_required: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recovery_status: Option<String>,
}

impl Serialize for GoalProof {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let delivery_metadata = sidecar::remembered_goal_proof_delivery_metadata(self);
        let review_artifacts = sidecar::remembered_goal_proof_review_artifacts(self);
        let integration_evidence = sidecar::remembered_goal_proof_integration_evidence(self);
        let oracle_evidence = sidecar::remembered_goal_proof_oracle_evidence(self);
        let mut field_count = 14;
        if review_artifacts.is_some() {
            field_count += 1;
        }
        if integration_evidence.is_some() {
            field_count += 1;
        }
        if oracle_evidence.is_some() {
            field_count += 1;
        }
        if self.git.is_some() {
            field_count += 1;
        }
        if delivery_metadata.is_some() {
            field_count += 1;
        }
        if self.recovery_status.is_some() {
            field_count += 1;
        }

        let mut state = serializer.serialize_struct("GoalProof", field_count)?;
        state.serialize_field("version", &self.version)?;
        state.serialize_field("goal_id", &self.goal_id)?;
        state.serialize_field("status", &self.status)?;
        state.serialize_field("readiness", &self.readiness)?;
        state.serialize_field("summary", &self.summary)?;
        state.serialize_field("generated_at", &self.generated_at)?;
        state.serialize_field("artifacts", &self.artifacts)?;
        state.serialize_field("task_graph_summary", &self.task_graph_summary)?;
        if let Some(delivery_metadata) = delivery_metadata {
            state.serialize_field("delivery_metadata", &delivery_metadata)?;
        }
        if let Some(review_artifacts) = review_artifacts {
            state.serialize_field("review_artifacts", &review_artifacts)?;
        }
        if let Some(integration_evidence) = integration_evidence {
            state.serialize_field("integration_evidence", &integration_evidence)?;
        }
        if let Some(oracle_evidence) = oracle_evidence {
            state.serialize_field("oracle_evidence", &oracle_evidence)?;
        }
        state.serialize_field("changed_files", &self.changed_files)?;
        state.serialize_field("commits", &self.commits)?;
        if let Some(git) = &self.git {
            state.serialize_field("git", git)?;
        }
        state.serialize_field("gates", &self.gates)?;
        state.serialize_field("post_mutation_gates_ran", &self.post_mutation_gates_ran)?;
        state.serialize_field("known_gaps", &self.known_gaps)?;
        state.serialize_field("human_decisions_required", &self.human_decisions_required)?;
        if let Some(recovery_status) = &self.recovery_status {
            state.serialize_field("recovery_status", recovery_status)?;
        }
        state.end()
    }
}

impl GoalProof {
    pub async fn load(goal_dir: &Path) -> Result<Self> {
        let path = goal_dir.join(GOAL_PROOF_FILE);
        let json = tokio::fs::read_to_string(&path)
            .await
            .with_context(|| format!("Failed to read goal proof: {}", path.display()))?;
        let value: Value = serde_json::from_str(&json)
            .with_context(|| format!("Failed to parse goal proof: {}", path.display()))?;
        let delivery_metadata = sidecar::proof_delivery_metadata_from_value(&value);
        let review_artifacts = sidecar::proof_review_artifacts_from_value(&value);
        let integration_evidence = sidecar::proof_integration_evidence_from_value(&value);
        let oracle_evidence = sidecar::proof_oracle_evidence_from_value(&value);
        let proof: Self = serde_json::from_value(value)
            .with_context(|| format!("Failed to parse goal proof: {}", path.display()))?;
        sidecar::remember_goal_proof_delivery_metadata(&proof, delivery_metadata);
        sidecar::remember_goal_proof_review_artifacts(&proof, review_artifacts);
        sidecar::remember_goal_proof_acceptance_evidence_for_value(
            &serde_json::to_value(&proof)?,
            integration_evidence,
            oracle_evidence,
        );
        Ok(proof)
    }
}

pub(crate) fn build_scaffold_proof(
    state: &GoalState,
    task_graph: &GoalTaskGraph,
    git: Option<super::evidence::GoalGitEvidence>,
    generated_at: DateTime<Utc>,
) -> GoalProof {
    let commits = proof_commits(&git);
    let controlled_by_state = status::state_status_controls_proof(state.status);
    let known_gaps = if controlled_by_state {
        Vec::new()
    } else {
        vec![
            "agent execution has not run for this goal yet".to_string(),
            "verification gates have not run for this goal".to_string(),
            "proof cannot claim readiness until agent-owned execution evidence exists".to_string(),
        ]
    };
    let mut proof = GoalProof {
        version: 1,
        goal_id: state.goal_id.clone(),
        status: GoalStatus::NotReady,
        readiness: "not ready: controller scaffold has not executed agents or verification gates"
            .to_string(),
        summary: format!(
            "Goal '{}' has durable planning artifacts, but no local verification or agent execution evidence yet.",
            state.normalized_goal
        ),
        generated_at,
        artifacts: state.artifacts.clone(),
        task_graph_summary: summarize_task_graph(task_graph),
        changed_files: Vec::new(),
        commits,
        git,
        gates: Vec::new(),
        post_mutation_gates_ran: false,
        known_gaps,
        human_decisions_required: Vec::new(),
        recovery_status: None,
    };
    status::reconcile_with_goal_state(&mut proof, state);
    proof
}

pub(crate) fn build_verified_proof(
    state: &GoalState,
    task_graph: &GoalTaskGraph,
    gates: Vec<GateResult>,
    changed_files: Vec<String>,
    git: Option<super::evidence::GoalGitEvidence>,
    post_mutation_gates_ran: bool,
    generated_at: DateTime<Utc>,
) -> GoalProof {
    let gates_ok = !gates.is_empty() && gates_passed(&gates);
    let agent_execution_done = goal_agent_execution_done(task_graph);
    let review_done = task_graph
        .tasks
        .iter()
        .any(|task| task.id == GOAL_REVIEW_TASK_ID && task.status == GoalTaskStatus::Done);
    let security_review_done = task_graph
        .tasks
        .iter()
        .any(|task| task.id == GOAL_SECURITY_REVIEW_TASK_ID && task.status == GoalTaskStatus::Done);
    let review_artifacts =
        collect_review_artifacts(review_done, security_review_done, &gates, &changed_files);
    let review_artifacts_ok = review::review_artifacts_passed(&review_artifacts);
    let commits = proof_commits(&git);
    let mut known_gaps = Vec::new();
    if !agent_execution_done {
        known_gaps.push("agent execution has not run for this goal yet".to_string());
        known_gaps.push(
            "proof cannot claim readiness until agent-owned execution evidence exists".to_string(),
        );
    }
    if agent_execution_done && !review_done {
        known_gaps.push("review evidence has not run for this goal yet".to_string());
    }
    if agent_execution_done && !security_review_done {
        known_gaps.push("security review evidence has not run for this goal yet".to_string());
    }
    known_gaps.extend(review::review_artifact_known_gaps(&review_artifacts));
    if agent_execution_done && !changed_files.is_empty() && !post_mutation_gates_ran {
        known_gaps
            .push("verification gates have not rerun after agent execution changes".to_string());
    }
    if agent_execution_done && review_done && security_review_done && changed_files.is_empty() {
        known_gaps.push(
            "project mutation and integration loop has not produced changed-file evidence yet"
                .to_string(),
        );
    }
    if agent_execution_done && review_done && security_review_done && !changed_files.is_empty() {
        known_gaps.push(
            "integration loop has not committed, opened a PR, or accepted the agent changes yet"
                .to_string(),
        );
    }

    if gates.is_empty() {
        known_gaps.push("no verification gates were detected or configured".to_string());
    } else if !gates_ok {
        known_gaps.push("required verification gates failed".to_string());
    }

    let proof_status = if status::state_status_controls_proof(state.status) {
        state.status
    } else {
        GoalStatus::NotReady
    };
    let readiness = if state.status == GoalStatus::Paused {
        "paused: execution was interrupted by operator request and can resume later".to_string()
    } else if state.status == GoalStatus::Cancelled {
        "cancelled: execution was interrupted by operator cancellation".to_string()
    } else if state.status == GoalStatus::NeedsMoreBudget {
        "needs more budget: execution stopped before spending beyond the configured budget"
            .to_string()
    } else if gates_ok
        && agent_execution_done
        && review_done
        && security_review_done
        && review_artifacts_ok
        && !changed_files.is_empty()
        && post_mutation_gates_ran
    {
        "not ready: agent changes passed verification, review, and security evidence, but integration acceptance is missing".to_string()
    } else if gates_ok
        && agent_execution_done
        && review_done
        && security_review_done
        && review_artifacts_ok
        && !changed_files.is_empty()
    {
        "not ready: agent changes exist, but verification and integration have not rerun after the mutation".to_string()
    } else if gates_ok
        && agent_execution_done
        && review_done
        && security_review_done
        && review_artifacts_ok
    {
        "not ready: verification, agent execution, review, and security evidence passed, but no project mutation was captured".to_string()
    } else if gates_ok && agent_execution_done && review_done && security_review_done {
        "not ready: required reviewer artifacts are incomplete or blocked".to_string()
    } else if gates_ok && agent_execution_done {
        "not ready: verification gates and bounded agent execution passed, but review/security evidence is missing".to_string()
    } else if gates_ok {
        "not ready: verification gates passed, but agent execution evidence is missing".to_string()
    } else {
        "not ready: required verification evidence is incomplete or failing".to_string()
    };

    let mut proof = GoalProof {
        version: 1,
        goal_id: state.goal_id.clone(),
        status: proof_status,
        readiness,
        summary: format!(
            "Goal '{}' has {} gate result(s) and remains not ready until all required execution and review evidence exists.",
            state.normalized_goal,
            gates.len()
        ),
        generated_at,
        artifacts: state.artifacts.clone(),
        task_graph_summary: summarize_task_graph(task_graph),
        changed_files,
        commits,
        git,
        gates,
        post_mutation_gates_ran,
        known_gaps,
        human_decisions_required: Vec::new(),
        recovery_status: None,
    };
    status::reconcile_with_goal_state(&mut proof, state);
    sidecar::remember_goal_proof_review_artifacts(&proof, review_artifacts);
    proof
}

pub(crate) fn reconcile_goal_proof_with_state(proof: &mut GoalProof, state: &GoalState) {
    status::reconcile_with_goal_state(proof, state);
}

fn proof_commits(git: &Option<super::evidence::GoalGitEvidence>) -> Vec<String> {
    git.as_ref()
        .map(|evidence| vec![evidence.head.clone()])
        .unwrap_or_default()
}

pub(crate) fn carry_goal_proof_sidecars(
    from: &GoalProof,
    to: &GoalProof,
    integration_evidence: Value,
    oracle_evidence: Value,
) {
    if let Some(delivery_metadata) = sidecar::remembered_goal_proof_delivery_metadata(from) {
        sidecar::remember_goal_proof_delivery_metadata(to, delivery_metadata);
    }
    if let Some(review_artifacts) = sidecar::remembered_goal_proof_review_artifacts(from) {
        sidecar::remember_goal_proof_review_artifacts(to, review_artifacts);
    }
    sidecar::remember_goal_proof_acceptance_evidence(to, integration_evidence, oracle_evidence);
}

#[cfg(test)]
mod tests;
