use anyhow::Result;
use chrono::Utc;
use serde_json::json;
use std::path::PathBuf;

use crate::runtime::goal::proof::GoalProof;
use crate::runtime::goal::state::{self, FileSystemGoalStateStore, GoalStateStore};
use crate::runtime::goal::types::{
    GoalControllerStep, GoalControllerStepKind, GoalRunUntilReadyOutcome,
};
use crate::runtime::goal::{evidence, proof};

#[derive(Debug, Clone)]
pub(crate) struct UntilReadyBlocker {
    pub(crate) reason: String,
    pub(crate) artifact_file: &'static str,
    pub(crate) human_decision_required: bool,
}

impl UntilReadyBlocker {
    pub(crate) fn policy(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
            artifact_file: super::UNTIL_READY_BLOCKER_FILE,
            human_decision_required: false,
        }
    }

    pub(crate) fn human(reason: impl Into<String>, artifact_file: &'static str) -> Self {
        Self {
            reason: reason.into(),
            artifact_file,
            human_decision_required: true,
        }
    }
}

pub(crate) async fn finalize_until_ready_blocker(
    goal_id: &str,
    mut steps: Vec<GoalControllerStep>,
    blocker: UntilReadyBlocker,
) -> Result<GoalRunUntilReadyOutcome> {
    let mut state = crate::runtime::goal::resolve_goal(goal_id).await?;
    let mut proof = GoalProof::load(&state.state_dir).await?;
    let now = Utc::now();
    let relative_path = PathBuf::from(blocker.artifact_file);
    if let Some(parent) = relative_path.parent() {
        crate::runtime::config::ensure_private_dir(&state.state_dir.join(parent)).await?;
    }
    let artifact = json!({
        "status": "blocked",
        "reason": &blocker.reason,
        "delivery_policy": "local",
        "merge_policy": "manual",
        "github_mutation": false,
        "integrator_acceptance": "manual",
        "proof": state::GOAL_PROOF_FILE,
        "recorded_at": now,
    });
    proof::write_json_artifact(&state.state_dir.join(&relative_path), &artifact).await?;
    evidence::record_artifact_path_once(&mut state, "policy_blocker", relative_path.clone(), now);
    state.updated_at = now;
    FileSystemGoalStateStore::new().save(&state).await?;

    push_unique(&mut proof.known_gaps, blocker.reason.clone());
    if blocker.human_decision_required {
        push_unique(&mut proof.human_decisions_required, blocker.reason.clone());
    }
    proof.generated_at = now;
    proof.artifacts = state.artifacts.clone();
    proof::write_json_artifact(&state.state_dir.join(state::GOAL_PROOF_FILE), &proof).await?;

    steps.push(GoalControllerStep {
        kind: GoalControllerStepKind::Blocked,
        status: proof.status,
        summary: blocker.reason.clone(),
    });
    Ok(GoalRunUntilReadyOutcome {
        state,
        proof,
        steps,
        blocker: Some(blocker.reason),
        policy_evidence_path: Some(relative_path),
    })
}

pub(crate) fn push_unique(values: &mut Vec<String>, value: String) {
    if !values.iter().any(|existing| existing == &value) {
        values.push(value);
    }
}
