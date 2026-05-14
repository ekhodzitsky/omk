use super::GoalProof;
use crate::runtime::goal::state::{GoalState, GoalStatus};

pub(super) fn state_status_controls_proof(status: GoalStatus) -> bool {
    matches!(
        status,
        GoalStatus::BlockedOnHuman
            | GoalStatus::BlockedOnExternal
            | GoalStatus::NeedsMoreBudget
            | GoalStatus::FailedInfra
            | GoalStatus::Paused
            | GoalStatus::Cancelled
    )
}

pub(super) fn reconcile_with_goal_state(proof: &mut GoalProof, state: &GoalState) {
    if !state_status_controls_proof(state.status) {
        return;
    }

    proof.status = state.status;
    let reason = state
        .failure
        .as_ref()
        .map(|failure| failure.reason.trim())
        .filter(|reason| !reason.is_empty());
    proof.readiness = readiness_for_state(state.status, reason);
    proof.summary = summary_for_state(state, reason);

    if let Some(decision) = human_decision_for_state(state.status, reason) {
        push_unique(&mut proof.human_decisions_required, decision);
    }
    push_unique(
        &mut proof.known_gaps,
        known_gap_for_state(state.status, reason),
    );
}

fn readiness_for_state(status: GoalStatus, reason: Option<&str>) -> String {
    let reason = reason.unwrap_or_else(|| default_reason(status));
    match status {
        GoalStatus::BlockedOnHuman => {
            format!("blocked on human: {reason}")
        }
        GoalStatus::BlockedOnExternal => {
            format!("blocked on external dependency: {reason}")
        }
        GoalStatus::NeedsMoreBudget => {
            format!("needs more budget: {reason}")
        }
        GoalStatus::FailedInfra => {
            format!("failed infra: {reason}")
        }
        GoalStatus::Paused => {
            format!("paused: {reason}")
        }
        GoalStatus::Cancelled => {
            format!("cancelled: {reason}")
        }
        _ => "not ready: required verification evidence is incomplete or failing".to_string(),
    }
}

fn summary_for_state(state: &GoalState, reason: Option<&str>) -> String {
    let reason = reason.unwrap_or_else(|| default_reason(state.status));
    match state.status {
        GoalStatus::BlockedOnHuman => format!(
            "Goal '{}' needs a human-defined oracle before autonomous execution can continue.",
            state.normalized_goal
        ),
        GoalStatus::BlockedOnExternal => format!(
            "Goal '{}' is blocked on external evidence or credentials: {reason}",
            state.normalized_goal
        ),
        GoalStatus::NeedsMoreBudget => format!(
            "Goal '{}' stopped before exceeding its configured budget: {reason}",
            state.normalized_goal
        ),
        GoalStatus::FailedInfra => format!(
            "Goal '{}' hit an infrastructure failure before proof-backed readiness: {reason}",
            state.normalized_goal
        ),
        GoalStatus::Paused => format!(
            "Goal '{}' is paused and can be resumed by the operator.",
            state.normalized_goal
        ),
        GoalStatus::Cancelled => format!(
            "Goal '{}' was cancelled before proof-backed readiness: {reason}",
            state.normalized_goal
        ),
        _ => format!(
            "Goal '{}' remains not ready until required evidence exists.",
            state.normalized_goal
        ),
    }
}

fn human_decision_for_state(status: GoalStatus, reason: Option<&str>) -> Option<String> {
    match status {
        GoalStatus::BlockedOnHuman => {
            Some(reason.unwrap_or_else(|| default_reason(status)).to_string())
        }
        _ => None,
    }
}

fn known_gap_for_state(status: GoalStatus, reason: Option<&str>) -> String {
    let reason = reason.unwrap_or_else(|| default_reason(status));
    match status {
        GoalStatus::BlockedOnHuman => "goal oracle is not testable without a human decision",
        GoalStatus::BlockedOnExternal => reason,
        GoalStatus::NeedsMoreBudget => "budget exhausted before proof-backed readiness",
        GoalStatus::FailedInfra => reason,
        GoalStatus::Paused => "goal is paused before proof-backed readiness",
        GoalStatus::Cancelled => reason,
        _ => "required verification evidence is incomplete or failing",
    }
    .to_string()
}

fn default_reason(status: GoalStatus) -> &'static str {
    match status {
        GoalStatus::BlockedOnHuman => {
            "Define testable success criteria before autonomous goal execution."
        }
        GoalStatus::BlockedOnExternal => {
            "External evidence or credentials are required before execution can continue."
        }
        GoalStatus::NeedsMoreBudget => {
            "Execution stopped before spending beyond the configured budget."
        }
        GoalStatus::FailedInfra => "Infrastructure failed before proof-backed readiness.",
        GoalStatus::Paused => "Execution was interrupted by operator request and can resume later.",
        GoalStatus::Cancelled => "Execution was interrupted by operator cancellation.",
        _ => "Required verification evidence is incomplete or failing.",
    }
}

fn push_unique(values: &mut Vec<String>, value: String) {
    if !values.iter().any(|existing| existing == &value) {
        values.push(value);
    }
}
