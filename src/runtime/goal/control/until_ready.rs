use anyhow::Result;
use chrono::Utc;
use serde_json::json;
use std::path::{Path, PathBuf};

use super::super::proof::GoalProof;
use super::super::state::{self, GoalStatus};
use super::super::task_graph::{goal_task_done, GoalTaskGraph, GoalTaskStatus};
use super::super::types::{GoalControllerStep, GoalControllerStepKind, GoalRunUntilReadyOutcome};
use super::super::{evidence, proof};

const MAX_EXECUTE_PASSES: usize = 8;
const MANUAL_INTEGRATION_BLOCKER_FILE: &str = "artifacts/policy/manual-integration-blocker.json";
const UNTIL_READY_BLOCKER_FILE: &str = "artifacts/policy/until-ready-blocker.json";

#[derive(Debug, Clone)]
struct UntilReadyBlocker {
    reason: String,
    artifact_file: &'static str,
    human_decision_required: bool,
}

impl UntilReadyBlocker {
    fn policy(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
            artifact_file: UNTIL_READY_BLOCKER_FILE,
            human_decision_required: false,
        }
    }

    fn human(reason: impl Into<String>, artifact_file: &'static str) -> Self {
        Self {
            reason: reason.into(),
            artifact_file,
            human_decision_required: true,
        }
    }
}

pub(crate) async fn run_goal_until_ready(
    goal: &str,
    options: state::CreateGoalOptions,
    project_dir: &Path,
) -> Result<GoalRunUntilReadyOutcome> {
    let state = super::super::create_goal(goal, options).await?;
    let mut steps = vec![GoalControllerStep {
        kind: GoalControllerStepKind::Plan,
        status: state.status,
        summary: "created durable goal scaffold and planning artifacts".to_string(),
    }];
    let proof = GoalProof::load(&state.state_dir).await?;
    if state.status == GoalStatus::BlockedOnHuman {
        let blocker = state
            .failure
            .as_ref()
            .map(|failure| failure.reason.clone())
            .unwrap_or_else(|| "human decision required before execution".to_string());
        steps.push(GoalControllerStep {
            kind: GoalControllerStepKind::Blocked,
            status: state.status,
            summary: blocker.clone(),
        });
        return Ok(GoalRunUntilReadyOutcome {
            state,
            proof,
            steps,
            blocker: Some(blocker),
            policy_evidence_path: None,
        });
    }

    let verified = super::super::verify_goal(&state.goal_id, project_dir).await?;
    steps.push(GoalControllerStep {
        kind: GoalControllerStepKind::Verify,
        status: verified.status,
        summary: verification_summary(&verified),
    });
    if !verification_can_continue(&verified) {
        return finalize_until_ready_blocker(
            &state.goal_id,
            steps,
            UntilReadyBlocker::policy(verification_blocker(&verified)),
        )
        .await;
    }

    for pass in 1..=MAX_EXECUTE_PASSES {
        let executed = super::super::execute_goal(&state.goal_id, project_dir).await?;
        steps.push(GoalControllerStep {
            kind: GoalControllerStepKind::Execute,
            status: executed.status,
            summary: format!("ran controller execution pass {pass}"),
        });
        if !proof_can_continue(&executed) {
            return finalize_until_ready_blocker(
                &state.goal_id,
                steps,
                terminal_blocker(&executed),
            )
            .await;
        }
        if !has_pending_agent_dispatch(&state.goal_id).await? {
            break;
        }
        if pass == MAX_EXECUTE_PASSES {
            return finalize_until_ready_blocker(
                &state.goal_id,
                steps,
                UntilReadyBlocker::policy(
                    "execution stopped after the maximum controller follow-up passes; inspect pending task graph follow-ups",
                ),
            )
            .await;
        }
    }

    let reviewed = super::super::review_goal(&state.goal_id, project_dir).await?;
    steps.push(GoalControllerStep {
        kind: GoalControllerStepKind::Review,
        status: reviewed.status,
        summary: "attached controller review and security-review evidence".to_string(),
    });
    let blocker = readiness_blocker(&state.goal_id, &reviewed).await?;
    finalize_until_ready_blocker(&state.goal_id, steps, blocker).await
}

fn verification_summary(proof: &GoalProof) -> String {
    if proof.gates.is_empty() {
        return "no verification gates were detected or configured".to_string();
    }
    let passed = proof.gates.iter().filter(|gate| gate.passed).count();
    format!(
        "ran {} verification gate(s), {passed} passed",
        proof.gates.len()
    )
}

fn verification_can_continue(proof: &GoalProof) -> bool {
    !proof.gates.is_empty() && crate::runtime::gates::gates_passed(&proof.gates)
}

fn proof_can_continue(proof: &GoalProof) -> bool {
    matches!(proof.status, GoalStatus::NotReady | GoalStatus::Running)
        && verification_can_continue(proof)
}

fn verification_blocker(proof: &GoalProof) -> String {
    if proof.gates.is_empty() {
        return "verification blocked: no local gates were detected or configured".to_string();
    }
    let failed = proof
        .gates
        .iter()
        .filter(|gate| !gate.passed)
        .map(|gate| gate.name.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    format!("verification blocked: required gate(s) failed: {failed}")
}

fn terminal_blocker(proof: &GoalProof) -> UntilReadyBlocker {
    match proof.status {
        GoalStatus::NeedsMoreBudget => {
            UntilReadyBlocker::policy("budget exhausted before proof-backed readiness")
        }
        GoalStatus::Paused => {
            UntilReadyBlocker::policy("goal paused before proof-backed readiness")
        }
        GoalStatus::Cancelled => {
            UntilReadyBlocker::policy("goal cancelled before proof-backed readiness")
        }
        GoalStatus::BlockedOnHuman => UntilReadyBlocker::human(
            proof
                .human_decisions_required
                .first()
                .cloned()
                .unwrap_or_else(|| {
                    "human decision required before execution can continue".to_string()
                }),
            UNTIL_READY_BLOCKER_FILE,
        ),
        GoalStatus::BlockedOnExternal => {
            UntilReadyBlocker::policy("external dependency blocked goal execution")
        }
        GoalStatus::FailedInfra => {
            UntilReadyBlocker::policy("infrastructure failure blocked goal execution")
        }
        _ => UntilReadyBlocker::policy(verification_blocker(proof)),
    }
}

async fn has_pending_agent_dispatch(goal_id: &str) -> Result<bool> {
    let state = super::super::resolve_goal(goal_id).await?;
    let task_graph = GoalTaskGraph::load(&state.state_dir).await?;
    Ok(super::super::agent::goal_agent_dispatch_plan(&state, &task_graph).is_some())
}

async fn readiness_blocker(goal_id: &str, proof: &GoalProof) -> Result<UntilReadyBlocker> {
    let state = super::super::resolve_goal(goal_id).await?;
    let task_graph = GoalTaskGraph::load(&state.state_dir).await?;
    let blocked_tasks = task_graph
        .tasks
        .iter()
        .filter(|task| task.status == GoalTaskStatus::Blocked)
        .map(|task| task.id.as_str())
        .collect::<Vec<_>>();
    if !blocked_tasks.is_empty() {
        return Ok(UntilReadyBlocker::policy(format!(
            "blocked task(s): {}; inspect task-graph.json and proof.json for policy evidence",
            blocked_tasks.join(", ")
        )));
    }
    if let Some(blocker) = review_wall_blocker(proof) {
        return Ok(UntilReadyBlocker::policy(blocker));
    }
    if manual_integration_acceptance_required(&task_graph, proof) {
        return Ok(UntilReadyBlocker::human(
            "manual integration acceptance is required before ready; local delivery policy keeps GitHub mutation and merge disabled"
                .to_string(),
            MANUAL_INTEGRATION_BLOCKER_FILE,
        ));
    }
    Ok(UntilReadyBlocker::policy(
        proof
            .known_gaps
            .first()
            .cloned()
            .unwrap_or_else(|| "proof remains not_ready without a ready claim".to_string()),
    ))
}

fn review_wall_blocker(proof: &GoalProof) -> Option<String> {
    proof
        .known_gaps
        .iter()
        .find(|gap| gap.contains("review is blocked") || gap.contains("review artifact"))
        .cloned()
}

fn manual_integration_acceptance_required(task_graph: &GoalTaskGraph, proof: &GoalProof) -> bool {
    goal_task_done(task_graph, state::GOAL_LOCAL_VERIFY_TASK_ID)
        && goal_task_done(task_graph, state::GOAL_AGENT_EXECUTE_TASK_ID)
        && goal_task_done(task_graph, state::GOAL_REVIEW_TASK_ID)
        && goal_task_done(task_graph, state::GOAL_SECURITY_REVIEW_TASK_ID)
        && !proof.changed_files.is_empty()
        && proof.post_mutation_gates_ran
}

async fn finalize_until_ready_blocker(
    goal_id: &str,
    mut steps: Vec<GoalControllerStep>,
    blocker: UntilReadyBlocker,
) -> Result<GoalRunUntilReadyOutcome> {
    let mut state = super::super::resolve_goal(goal_id).await?;
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
    state.save().await?;

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

fn push_unique(values: &mut Vec<String>, value: String) {
    if !values.iter().any(|existing| existing == &value) {
        values.push(value);
    }
}
