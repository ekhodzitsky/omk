use anyhow::Result;
use std::path::{Path, PathBuf};

use crate::runtime::events::{Event, EventBuilder, EventKind, EventWriter, RunId};
use crate::runtime::goal::delivery::GoalDeliveryPolicy;
use crate::runtime::goal::proof::GoalProof;
use crate::runtime::goal::state::{self, GoalStatus};
use crate::runtime::goal::task_graph::{
    all_slices_done, goal_task_done, GoalTaskGraph, GoalTaskStatus,
};
use crate::runtime::goal::types::{
    GoalControllerStep, GoalControllerStepKind, GoalRunUntilReadyOutcome,
};
use crate::runtime::goal::worktree::remove_goal_worktrees;

const MAX_EXECUTE_PASSES: usize = 8;
const MANUAL_INTEGRATION_BLOCKER_FILE: &str = "artifacts/policy/manual-integration-blocker.json";
const UNTIL_READY_BLOCKER_FILE: &str = "artifacts/policy/until-ready-blocker.json";

mod blocker;
mod delivery;
mod gates;
mod git;
mod integrator;

pub(crate) use blocker::UntilReadyBlocker;
pub(crate) use git::resolve_base_branch;

pub(crate) async fn run_goal_until_ready(
    goal: &str,
    options: state::CreateGoalOptions,
    project_dir: &Path,
) -> Result<GoalRunUntilReadyOutcome> {
    let state = crate::runtime::goal::create_goal(goal, options.clone(), None).await?;
    let events_path = state.state_dir.join(crate::runtime::config::EVENTS_FILE);
    let event_writer = EventWriter::new(events_path);
    let event_builder = EventBuilder::new(RunId(state.goal_id.clone()));

    let plan_step = GoalControllerStep {
        kind: GoalControllerStepKind::Plan,
        status: state.status,
        summary: "created durable goal scaffold and planning artifacts".to_string(),
    };
    emit_narrative(
        &event_writer,
        &event_builder,
        &RunId(state.goal_id.clone()),
        &plan_step.summary,
    )
    .await;
    let mut steps = vec![plan_step];

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
        emit_narrative(
            &event_writer,
            &event_builder,
            &RunId(state.goal_id.clone()),
            &format!("blocked: {blocker}"),
        )
        .await;
        return Ok(GoalRunUntilReadyOutcome {
            state,
            proof,
            steps,
            blocker: Some(blocker),
            policy_evidence_path: None,
        });
    }

    let verified = crate::runtime::goal::verify_goal(&state.goal_id, project_dir).await?;
    let verify_summary = verification_summary(&verified);
    steps.push(GoalControllerStep {
        kind: GoalControllerStepKind::Verify,
        status: verified.status,
        summary: verify_summary.clone(),
    });
    emit_narrative(
        &event_writer,
        &event_builder,
        &RunId(state.goal_id.clone()),
        &format!("verify: {verify_summary}"),
    )
    .await;
    if !verification_can_continue(&verified) {
        return blocker::finalize_until_ready_blocker(
            &state.goal_id,
            steps,
            UntilReadyBlocker::policy(verification_blocker(&verified)),
        )
        .await;
    }

    for pass in 1..=MAX_EXECUTE_PASSES {
        let executed = crate::runtime::goal::execute_goal(&state.goal_id, project_dir).await?;
        let exec_summary = format!("execution pass {pass}: {}", executed.summary);
        steps.push(GoalControllerStep {
            kind: GoalControllerStepKind::Execute,
            status: executed.status,
            summary: exec_summary.clone(),
        });
        emit_narrative(
            &event_writer,
            &event_builder,
            &RunId(state.goal_id.clone()),
            &exec_summary,
        )
        .await;
        if !proof_can_continue(&executed) {
            return blocker::finalize_until_ready_blocker(
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
            return blocker::finalize_until_ready_blocker(
                &state.goal_id,
                steps,
                UntilReadyBlocker::policy(
                    "execution stopped after the maximum controller follow-up passes; inspect pending task graph follow-ups",
                ),
            )
            .await;
        }
    }

    let reviewed = crate::runtime::goal::review_goal(&state.goal_id, project_dir).await?;
    steps.push(GoalControllerStep {
        kind: GoalControllerStepKind::Review,
        status: reviewed.status,
        summary: "attached controller review and security-review evidence".to_string(),
    });
    emit_narrative(
        &event_writer,
        &event_builder,
        &RunId(state.goal_id.clone()),
        "review completed",
    )
    .await;
    let blocker = readiness_blocker(&state.goal_id, &reviewed).await?;
    if blocker.reason.contains("manual integration acceptance")
        && options.delivery_policy != GoalDeliveryPolicy::Local
    {
        if state.slice_execution && options.delivery_policy != GoalDeliveryPolicy::Local {
            return integrator::finalize_slice_integrator(
                &state.goal_id,
                steps,
                options.delivery_policy,
                options.merge_policy,
                options.enforce_protection,
                project_dir,
            )
            .await;
        }
        return delivery::finalize_until_ready_delivery(
            &state.goal_id,
            steps,
            options.delivery_policy,
            options.merge_policy,
            options.enforce_protection,
            project_dir,
        )
        .await;
    }
    blocker::finalize_until_ready_blocker(&state.goal_id, steps, blocker).await
}

pub(crate) fn verification_summary(proof: &GoalProof) -> String {
    if proof.gates.is_empty() {
        return "no verification gates were detected or configured".to_string();
    }
    let passed = proof.gates.iter().filter(|gate| gate.passed).count();
    format!(
        "ran {} verification gate(s), {passed} passed",
        proof.gates.len()
    )
}

pub(crate) fn verification_can_continue(proof: &GoalProof) -> bool {
    !proof.gates.is_empty() && crate::runtime::gates::gates_passed(&proof.gates)
}

pub(crate) fn proof_can_continue(proof: &GoalProof) -> bool {
    matches!(proof.status, GoalStatus::NotReady | GoalStatus::Running)
        && verification_can_continue(proof)
}

pub(crate) fn verification_blocker(proof: &GoalProof) -> String {
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

pub(crate) fn terminal_blocker(proof: &GoalProof) -> UntilReadyBlocker {
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
    let state = crate::runtime::goal::resolve_goal(goal_id).await?;
    let task_graph = GoalTaskGraph::load(&state.state_dir).await?;
    if state.slice_execution {
        let done = all_slices_done(&state.state_dir, &task_graph).await?;
        return Ok(!done);
    }
    Ok(crate::runtime::goal::agent::goal_agent_dispatch_plan(&state, &task_graph).is_some())
}

async fn readiness_blocker(goal_id: &str, proof: &GoalProof) -> Result<UntilReadyBlocker> {
    let state = crate::runtime::goal::resolve_goal(goal_id).await?;
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

pub(crate) fn review_wall_blocker(proof: &GoalProof) -> Option<String> {
    proof
        .known_gaps
        .iter()
        .find(|gap| gap.contains("review is blocked") || gap.contains("review artifact"))
        .cloned()
}

pub(crate) fn manual_integration_acceptance_required(
    task_graph: &crate::runtime::goal::task_graph::GoalTaskGraph,
    proof: &GoalProof,
) -> bool {
    goal_task_done(task_graph, state::GOAL_LOCAL_VERIFY_TASK_ID)
        && crate::runtime::goal::task_graph::goal_agent_execution_done(task_graph)
        && goal_task_done(task_graph, state::GOAL_REVIEW_TASK_ID)
        && goal_task_done(task_graph, state::GOAL_SECURITY_REVIEW_TASK_ID)
        && !proof.changed_files.is_empty()
        && proof.post_mutation_gates_ran
}

pub(super) async fn cleanup_goal_worktrees(state: &state::GoalState, project_dir: &Path) {
    if !state.slice_execution {
        return;
    }
    if let Ok(records) =
        crate::runtime::goal::task_graph::load_goal_task_delivery_records(&state.state_dir).await
    {
        let paths: Vec<PathBuf> = records
            .into_iter()
            .filter_map(|r| r.metadata.worktree_path)
            .collect();
        remove_goal_worktrees(project_dir, &paths).await;
    }
}

async fn emit_narrative(
    writer: &EventWriter,
    _builder: &EventBuilder,
    run_id: &RunId,
    message: &str,
) {
    if let Ok(event) = Event::new(run_id.clone(), EventKind::TaskOutput)
        .with_actor("controller")
        .with_message(message)
    {
        let _ = writer.append(&event).await;
    }
}

#[cfg(test)]
mod tests;
