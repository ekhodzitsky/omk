use anyhow::Result;
use chrono::Utc;

use super::state::{GoalFailure, GoalState, GoalStatus};
use super::{budget, state};
use crate::runtime::goal::state::{FileSystemGoalStateStore, GoalStateStore};

mod until_ready;

pub(crate) use until_ready::run_goal_until_ready;

pub async fn pause_goal(goal_id: &str) -> Result<GoalState> {
    let mut state = super::resolve_goal(goal_id).await?;
    if matches!(state.status, GoalStatus::Ready | GoalStatus::Cancelled) {
        anyhow::bail!(
            "Goal '{}' is terminal ({}) and cannot be paused",
            state.goal_id,
            state.status
        );
    }

    let now = Utc::now();
    state.status = GoalStatus::Paused;
    state.updated_at = now;
    state.completed_at = None;
    FileSystemGoalStateStore::new().save(&state).await?;
    append_goal_lifecycle_event(&state, crate::runtime::events::EventKind::GoalPaused).await?;
    budget::append_budget_checkpoint(&state, "goal_paused").await?;
    Ok(state)
}

pub async fn resume_goal(goal_id: &str) -> Result<GoalState> {
    let mut state = super::resolve_goal(goal_id).await?;
    if state.status != GoalStatus::Paused {
        anyhow::bail!(
            "Goal '{}' is not paused (status: {})",
            state.goal_id,
            state.status
        );
    }

    let now = Utc::now();
    state.status = GoalStatus::NotReady;
    state.updated_at = now;
    state.completed_at = None;
    FileSystemGoalStateStore::new().save(&state).await?;
    append_goal_lifecycle_event(&state, crate::runtime::events::EventKind::GoalResumed).await?;
    budget::append_budget_checkpoint(&state, "goal_resumed").await?;
    Ok(state)
}

pub async fn cancel_goal(goal_id: &str) -> Result<GoalState> {
    let mut state = super::resolve_goal(goal_id).await?;
    let now = Utc::now();
    state.status = GoalStatus::Cancelled;
    state.updated_at = now;
    state.completed_at = Some(now);
    state.failure = Some(GoalFailure {
        reason: "cancelled by user".to_string(),
        recorded_at: now,
    });
    FileSystemGoalStateStore::new().save(&state).await?;

    let failure_json = serde_json::to_string_pretty(&state)?;
    crate::runtime::atomic::atomic_write(
        &state.state_dir.join(state::GOAL_FAILURE_FILE),
        failure_json.as_bytes(),
    )
    .await?;

    let writer = crate::runtime::events::EventWriter::new(
        state.state_dir.join(crate::runtime::config::EVENTS_FILE),
    );
    let run_id = crate::runtime::events::RunId(state.goal_id.clone());
    let interrupted = crate::runtime::events::Event::new(
        run_id.clone(),
        crate::runtime::events::EventKind::ManualInterrupt,
    )
    .with_actor("omk-cli");
    let failed =
        crate::runtime::events::EventBuilder::new(run_id).run_failed("cancelled by user")?;
    writer.append_many(&[interrupted, failed]).await?;
    budget::append_budget_checkpoint(&state, "goal_cancelled").await?;

    Ok(state)
}

async fn append_goal_lifecycle_event(
    state: &GoalState,
    kind: crate::runtime::events::EventKind,
) -> Result<()> {
    let writer = crate::runtime::events::EventWriter::new(
        state.state_dir.join(crate::runtime::config::EVENTS_FILE),
    );
    let event = crate::runtime::events::Event::new(
        crate::runtime::events::RunId(state.goal_id.clone()),
        kind,
    )
    .with_actor("omk-cli")
    .with_payload(serde_json::json!({
        "status": state.status.to_string(),
        "phase": state.phase.to_string(),
        "updated_at": state.updated_at,
    }))?;
    writer.append(&event).await
}
