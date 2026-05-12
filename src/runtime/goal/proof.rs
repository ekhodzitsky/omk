use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::Path;

use super::state::{
    GoalState, GoalStatus, GOAL_AGENT_EXECUTE_TASK_ID, GOAL_PROOF_FILE, GOAL_REVIEW_TASK_ID,
    GOAL_SECURITY_REVIEW_TASK_ID,
};
use super::task_graph::{
    summarize_task_graph, GoalTaskGraph, GoalTaskGraphSummary, GoalTaskStatus,
};
use crate::runtime::gates::{gates_passed, GateResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
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
}

impl GoalProof {
    pub async fn load(goal_dir: &Path) -> Result<Self> {
        let path = goal_dir.join(GOAL_PROOF_FILE);
        let json = tokio::fs::read_to_string(&path)
            .await
            .with_context(|| format!("Failed to read goal proof: {}", path.display()))?;
        let proof = serde_json::from_str(&json)
            .with_context(|| format!("Failed to parse goal proof: {}", path.display()))?;
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
    GoalProof {
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
        known_gaps: vec![
            "agent execution has not run for this goal yet".to_string(),
            "verification gates have not run for this goal".to_string(),
            "proof cannot claim readiness until agent-owned execution evidence exists".to_string(),
        ],
        human_decisions_required: Vec::new(),
    }
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
    let agent_execution_done = task_graph
        .tasks
        .iter()
        .any(|task| task.id == GOAL_AGENT_EXECUTE_TASK_ID && task.status == GoalTaskStatus::Done);
    let review_done = task_graph
        .tasks
        .iter()
        .any(|task| task.id == GOAL_REVIEW_TASK_ID && task.status == GoalTaskStatus::Done);
    let security_review_done = task_graph
        .tasks
        .iter()
        .any(|task| task.id == GOAL_SECURITY_REVIEW_TASK_ID && task.status == GoalTaskStatus::Done);
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

    GoalProof {
        version: 1,
        goal_id: state.goal_id.clone(),
        status: GoalStatus::NotReady,
        readiness: if gates_ok
            && agent_execution_done
            && review_done
            && security_review_done
            && !changed_files.is_empty()
            && post_mutation_gates_ran
        {
            "not ready: agent changes passed verification, review, and security evidence, but integration acceptance is missing".to_string()
        } else if gates_ok
            && agent_execution_done
            && review_done
            && security_review_done
            && !changed_files.is_empty()
        {
            "not ready: agent changes exist, but verification and integration have not rerun after the mutation".to_string()
        } else if gates_ok && agent_execution_done && review_done && security_review_done {
            "not ready: verification, agent execution, review, and security evidence passed, but no project mutation was captured".to_string()
        } else if gates_ok && agent_execution_done {
            "not ready: verification gates and bounded agent execution passed, but review/security evidence is missing".to_string()
        } else if gates_ok {
            "not ready: verification gates passed, but agent execution evidence is missing"
                .to_string()
        } else {
            "not ready: required verification evidence is incomplete or failing".to_string()
        },
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
    }
}

fn proof_commits(git: &Option<super::evidence::GoalGitEvidence>) -> Vec<String> {
    git.as_ref()
        .map(|evidence| vec![evidence.head.clone()])
        .unwrap_or_default()
}

pub(crate) async fn write_json_artifact<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let json = serde_json::to_string_pretty(value)?;
    crate::runtime::atomic::atomic_write(path, json.as_bytes()).await
}
