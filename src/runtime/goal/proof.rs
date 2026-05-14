use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize, Serializer};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

use super::state::{
    GoalState, GoalStatus, GOAL_AGENT_EXECUTE_TASK_ID, GOAL_PROOF_FILE, GOAL_REVIEW_TASK_ID,
    GOAL_SECURITY_REVIEW_TASK_ID, GOAL_TASK_GRAPH_FILE,
};
use super::task_graph::{
    summarize_task_graph, GoalTaskGraph, GoalTaskGraphSummary, GoalTaskStatus,
};
use crate::runtime::gates::{gates_passed, GateResult};

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
}

impl Serialize for GoalProof {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let delivery_metadata = remembered_goal_proof_delivery_metadata(self);
        let mut field_count = 14;
        if self.git.is_some() {
            field_count += 1;
        }
        if delivery_metadata.is_some() {
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
        state.serialize_field("changed_files", &self.changed_files)?;
        state.serialize_field("commits", &self.commits)?;
        if let Some(git) = &self.git {
            state.serialize_field("git", git)?;
        }
        state.serialize_field("gates", &self.gates)?;
        state.serialize_field("post_mutation_gates_ran", &self.post_mutation_gates_ran)?;
        state.serialize_field("known_gaps", &self.known_gaps)?;
        state.serialize_field("human_decisions_required", &self.human_decisions_required)?;
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
        let delivery_metadata = proof_delivery_metadata_from_value(&value);
        let proof: Self = serde_json::from_value(value)
            .with_context(|| format!("Failed to parse goal proof: {}", path.display()))?;
        remember_goal_proof_delivery_metadata(&proof, delivery_metadata);
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

fn proof_delivery_metadata_from_value(value: &Value) -> Vec<Value> {
    value
        .get("delivery_metadata")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .cloned()
        .collect()
}

// Artifact-only metadata stays out of GoalProof to preserve public struct literals.
static LOADED_PROOF_METADATA: OnceLock<Mutex<HashMap<String, Vec<Value>>>> = OnceLock::new();

fn proof_cache_key(proof: &GoalProof) -> String {
    let version = proof.version.to_string();
    let status = proof.status.to_string();
    proof_cache_key_parts(&[
        &version,
        &proof.goal_id,
        &status,
        &proof.readiness,
        &proof.summary,
    ])
}

fn proof_cache_key_from_value(value: &Value) -> Option<String> {
    let version = value.get("version")?.as_u64()?.to_string();
    let goal_id = value.get("goal_id")?.as_str()?;
    let status = value.get("status")?.as_str()?;
    let readiness = value.get("readiness")?.as_str()?;
    let summary = value.get("summary")?.as_str()?;
    Some(proof_cache_key_parts(&[
        &version, goal_id, status, readiness, summary,
    ]))
}

fn proof_cache_key_parts(parts: &[&str]) -> String {
    parts.join("\n")
}

fn remember_goal_proof_delivery_metadata_for_value(
    proof_value: &Value,
    delivery_metadata: Vec<Value>,
) {
    let Some(key) = proof_cache_key_from_value(proof_value) else {
        return;
    };
    remember_goal_proof_delivery_metadata_with_key(key, delivery_metadata);
}

fn remember_goal_proof_delivery_metadata(proof: &GoalProof, delivery_metadata: Vec<Value>) {
    remember_goal_proof_delivery_metadata_with_key(proof_cache_key(proof), delivery_metadata);
}

fn remember_goal_proof_delivery_metadata_with_key(key: String, delivery_metadata: Vec<Value>) {
    let cache = LOADED_PROOF_METADATA.get_or_init(|| Mutex::new(HashMap::new()));
    let Ok(mut cache) = cache.lock() else {
        return;
    };
    if delivery_metadata.is_empty() {
        cache.remove(&key);
    } else {
        cache.insert(key, delivery_metadata);
    }
}

fn remembered_goal_proof_delivery_metadata(proof: &GoalProof) -> Option<Vec<Value>> {
    let cache = LOADED_PROOF_METADATA.get_or_init(|| Mutex::new(HashMap::new()));
    let Ok(cache) = cache.lock() else {
        return None;
    };
    cache.get(&proof_cache_key(proof)).cloned()
}

pub(crate) async fn write_json_artifact<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let mut value = serde_json::to_value(value)?;
    enrich_goal_json_artifact(path, &mut value).await?;
    let json = serde_json::to_string_pretty(&value)?;
    crate::runtime::atomic::atomic_write(path, json.as_bytes()).await
}

async fn enrich_goal_json_artifact(path: &Path, value: &mut Value) -> Result<()> {
    let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
        return Ok(());
    };
    let Some(goal_dir) = path.parent() else {
        return Ok(());
    };

    match file_name {
        GOAL_TASK_GRAPH_FILE => {
            super::task_graph::preserve_delivery_metadata_in_value(goal_dir, value).await
        }
        GOAL_PROOF_FILE => attach_delivery_metadata_to_proof_value(goal_dir, value).await,
        _ => Ok(()),
    }
}

async fn attach_delivery_metadata_to_proof_value(
    goal_dir: &Path,
    proof_value: &mut Value,
) -> Result<()> {
    let delivery_metadata = super::task_graph::load_task_delivery_metadata(goal_dir).await?;
    remember_goal_proof_delivery_metadata_for_value(proof_value, delivery_metadata.clone());
    if delivery_metadata.is_empty() {
        return Ok(());
    }
    if let Some(proof) = proof_value.as_object_mut() {
        proof.insert(
            "delivery_metadata".to_string(),
            Value::Array(delivery_metadata),
        );
    }
    Ok(())
}
