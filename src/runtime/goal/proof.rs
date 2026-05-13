use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::Path;

use super::state::{
    GoalState, GoalStatus, GOAL_AGENT_EXECUTE_TASK_ID, GOAL_ARTIFACTS_DIR, GOAL_PROOF_FILE,
    GOAL_REVIEW_ARTIFACTS_DIR, GOAL_REVIEW_FILE, GOAL_REVIEW_TASK_ID, GOAL_SECURITY_REVIEW_FILE,
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub review_artifacts: Vec<Value>,
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
    let human_decisions_required = if state.status == GoalStatus::BlockedOnHuman {
        state
            .failure
            .as_ref()
            .map(|failure| vec![failure.reason.clone()])
            .unwrap_or_else(|| {
                vec![
                    "Define testable success criteria before autonomous goal execution."
                        .to_string(),
                ]
            })
    } else {
        Vec::new()
    };
    let known_gaps = if state.status == GoalStatus::BlockedOnHuman {
        vec!["goal oracle is not testable without a human decision".to_string()]
    } else {
        vec![
            "agent execution has not run for this goal yet".to_string(),
            "verification gates have not run for this goal".to_string(),
            "proof cannot claim readiness until agent-owned execution evidence exists".to_string(),
        ]
    };
    let readiness = if state.status == GoalStatus::BlockedOnHuman {
        "blocked on human: testable success criteria are required before autonomous execution"
            .to_string()
    } else {
        "not ready: controller scaffold has not executed agents or verification gates".to_string()
    };
    let summary = if state.status == GoalStatus::BlockedOnHuman {
        format!(
            "Goal '{}' needs a human-defined oracle before autonomous execution can continue.",
            state.normalized_goal
        )
    } else {
        format!(
            "Goal '{}' has durable planning artifacts, but no local verification or agent execution evidence yet.",
            state.normalized_goal
        )
    };
    GoalProof {
        version: 1,
        goal_id: state.goal_id.clone(),
        status: match state.status {
            GoalStatus::BlockedOnHuman => GoalStatus::BlockedOnHuman,
            _ => GoalStatus::NotReady,
        },
        readiness,
        summary,
        generated_at,
        artifacts: state.artifacts.clone(),
        task_graph_summary: summarize_task_graph(task_graph),
        review_artifacts: Vec::new(),
        changed_files: Vec::new(),
        commits,
        git,
        gates: Vec::new(),
        post_mutation_gates_ran: false,
        known_gaps,
        human_decisions_required,
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
    let review_artifacts =
        collect_review_artifacts(review_done, security_review_done, &gates, &changed_files);
    let review_artifacts_ok = review_artifacts_passed(&review_artifacts);
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
    known_gaps.extend(review_artifact_known_gaps(&review_artifacts));
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

    let proof_status = match state.status {
        GoalStatus::Paused | GoalStatus::Cancelled | GoalStatus::NeedsMoreBudget => state.status,
        _ => GoalStatus::NotReady,
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

    GoalProof {
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
        review_artifacts,
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

fn collect_review_artifacts(
    review_done: bool,
    security_review_done: bool,
    gates: &[GateResult],
    changed_files: &[String],
) -> Vec<Value> {
    if !review_done && !security_review_done {
        return Vec::new();
    }

    let gates_ok = !gates.is_empty() && gates_passed(gates);
    let performance_ok = gates
        .iter()
        .filter(|gate| is_performance_gate(&gate.name))
        .any(|gate| gate.passed);
    let review_path =
        format!("{GOAL_ARTIFACTS_DIR}/{GOAL_REVIEW_ARTIFACTS_DIR}/{GOAL_REVIEW_FILE}");
    let security_path =
        format!("{GOAL_ARTIFACTS_DIR}/{GOAL_REVIEW_ARTIFACTS_DIR}/{GOAL_SECURITY_REVIEW_FILE}");

    vec![
        review_artifact(
            "architect",
            review_done,
            &review_path,
            "architecture review artifact is present",
            "architect review artifact is missing",
        ),
        review_artifact(
            "code",
            review_done && !changed_files.is_empty(),
            &review_path,
            "code review has changed-file evidence to inspect",
            "code review is blocked until changed-file evidence exists",
        ),
        review_artifact(
            "test",
            gates_ok,
            &review_path,
            "test review passed because required verification gates passed",
            "test review is blocked until required verification gates pass",
        ),
        review_artifact(
            "security",
            security_review_done,
            &security_path,
            "security review artifact is present",
            "security review artifact is missing",
        ),
        review_artifact(
            "performance",
            performance_ok,
            &review_path,
            "performance review passed because a performance/benchmark gate passed",
            "performance review is blocked until performance or benchmark gate evidence exists",
        ),
    ]
}

fn review_artifact(
    pass: &str,
    passed: bool,
    path: &str,
    passed_summary: &str,
    blocked_gap: &str,
) -> Value {
    if passed {
        json!({
            "pass": pass,
            "status": "passed",
            "path": path,
            "summary": passed_summary,
        })
    } else {
        json!({
            "pass": pass,
            "status": "blocked",
            "path": path,
            "summary": blocked_gap,
            "known_gaps": [blocked_gap],
        })
    }
}

fn review_artifact_known_gaps(artifacts: &[Value]) -> Vec<String> {
    artifacts
        .iter()
        .filter_map(|artifact| artifact.get("known_gaps").and_then(Value::as_array))
        .flat_map(|gaps| gaps.iter().filter_map(Value::as_str).map(str::to_string))
        .collect()
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

fn is_performance_gate(name: &str) -> bool {
    let normalized = name.to_ascii_lowercase();
    normalized.contains("perf") || normalized.contains("bench")
}

pub(crate) async fn write_json_artifact<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let json = serde_json::to_string_pretty(value)?;
    crate::runtime::atomic::atomic_write(path, json.as_bytes()).await
}
