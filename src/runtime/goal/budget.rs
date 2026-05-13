use anyhow::{bail, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::warn;

use super::state::{
    parse_goal_duration_secs, GoalPhase, GoalState, GoalStatus, GOAL_BUDGET_CHECKPOINTS_FILE,
    GOAL_CONTROLLER_ACTOR,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalBudgetCheckpoint {
    pub version: u32,
    pub goal_id: String,
    pub label: String,
    pub status: GoalStatus,
    pub phase: GoalPhase,
    pub recorded_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget_time: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_budget_secs: Option<u64>,
    pub elapsed_since_created_secs: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remaining_budget_secs: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalBudgetReport {
    pub version: u32,
    pub goal_id: String,
    pub generated_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget_time: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_budget_secs: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest: Option<GoalBudgetCheckpoint>,
    pub checkpoints: Vec<GoalBudgetCheckpoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GoalBudgetExhaustedEvent {
    action: String,
    status: GoalStatus,
    phase: GoalPhase,
    recorded_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    budget_time: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    total_budget_secs: Option<u64>,
    pub elapsed_since_created_secs: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remaining_budget_secs: Option<u64>,
}

pub async fn goal_budget(goal_id: &str) -> Result<GoalBudgetReport> {
    let state = super::resolve_goal(goal_id).await?;
    let checkpoints = read_budget_checkpoints(&state).await?;
    Ok(GoalBudgetReport {
        version: 1,
        goal_id: state.goal_id,
        generated_at: Utc::now(),
        budget_time: state.budget_time.clone(),
        total_budget_secs: state
            .budget_time
            .as_deref()
            .and_then(parse_goal_duration_secs),
        latest: checkpoints.last().cloned(),
        checkpoints,
    })
}

pub(crate) async fn ensure_budget_available(state: &mut GoalState, action: &str) -> Result<()> {
    let now = Utc::now();
    let Some(total_budget_secs) = state
        .budget_time
        .as_deref()
        .and_then(parse_goal_duration_secs)
    else {
        return Ok(());
    };
    let elapsed_since_created_secs = now
        .signed_duration_since(state.created_at)
        .num_seconds()
        .max(0) as u64;
    if elapsed_since_created_secs < total_budget_secs {
        return Ok(());
    }

    state.status = GoalStatus::NeedsMoreBudget;
    state.updated_at = now;
    state.completed_at = Some(now);
    state.save().await?;
    append_budget_exhausted_event(
        state,
        &GoalBudgetExhaustedEvent {
            action: action.to_string(),
            status: state.status,
            phase: state.phase,
            recorded_at: now,
            budget_time: state.budget_time.clone(),
            total_budget_secs: Some(total_budget_secs),
            elapsed_since_created_secs,
            remaining_budget_secs: Some(0),
        },
    )
    .await?;
    append_budget_checkpoint(state, "budget_exhausted").await?;
    bail!(
        "Goal '{}' needs more budget before running `{}` (budget_time={}, elapsed={}s)",
        state.goal_id,
        action,
        state.budget_time.as_deref().unwrap_or("unbounded"),
        elapsed_since_created_secs
    );
}

pub(crate) async fn append_budget_checkpoint(
    state: &GoalState,
    label: &str,
) -> Result<GoalBudgetCheckpoint> {
    let checkpoint = build_budget_checkpoint(state, label, Utc::now());
    let line = serde_json::to_vec(&checkpoint)?;
    let mut content = line;
    content.push(b'\n');
    crate::runtime::atomic::atomic_append(&budget_checkpoints_path(state), &content).await?;
    append_budget_checkpoint_event(state, &checkpoint).await?;
    Ok(checkpoint)
}

async fn append_budget_exhausted_event(
    state: &GoalState,
    payload: &GoalBudgetExhaustedEvent,
) -> Result<()> {
    let writer = crate::runtime::events::EventWriter::new(
        state.state_dir.join(crate::runtime::config::EVENTS_FILE),
    );
    let event = crate::runtime::events::Event::new(
        crate::runtime::events::RunId(state.goal_id.clone()),
        crate::runtime::events::EventKind::GoalBudgetExhausted,
    )
    .with_actor(GOAL_CONTROLLER_ACTOR)
    .with_payload(payload)?;
    writer.append(&event).await
}

fn build_budget_checkpoint(
    state: &GoalState,
    label: &str,
    recorded_at: DateTime<Utc>,
) -> GoalBudgetCheckpoint {
    let total_budget_secs = state
        .budget_time
        .as_deref()
        .and_then(parse_goal_duration_secs);
    let elapsed_since_created_secs = recorded_at
        .signed_duration_since(state.created_at)
        .num_seconds()
        .max(0) as u64;
    let remaining_budget_secs =
        total_budget_secs.map(|total| total.saturating_sub(elapsed_since_created_secs));

    GoalBudgetCheckpoint {
        version: 1,
        goal_id: state.goal_id.clone(),
        label: label.to_string(),
        status: state.status,
        phase: state.phase,
        recorded_at,
        budget_time: state.budget_time.clone(),
        total_budget_secs,
        elapsed_since_created_secs,
        remaining_budget_secs,
    }
}

async fn read_budget_checkpoints(state: &GoalState) -> Result<Vec<GoalBudgetCheckpoint>> {
    let path = budget_checkpoints_path(state);
    let content = match tokio::fs::read_to_string(&path).await {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error.into()),
    };

    let mut checkpoints = Vec::new();
    for (line_no, line) in content.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        match serde_json::from_str::<GoalBudgetCheckpoint>(line) {
            Ok(checkpoint) => checkpoints.push(checkpoint),
            Err(error) => {
                warn!(
                    line = line_no + 1,
                    error = %error,
                    "Skipping malformed goal budget checkpoint"
                );
            }
        }
    }
    Ok(checkpoints)
}

async fn append_budget_checkpoint_event(
    state: &GoalState,
    checkpoint: &GoalBudgetCheckpoint,
) -> Result<()> {
    let writer = crate::runtime::events::EventWriter::new(
        state.state_dir.join(crate::runtime::config::EVENTS_FILE),
    );
    let event = crate::runtime::events::Event::new(
        crate::runtime::events::RunId(state.goal_id.clone()),
        crate::runtime::events::EventKind::BudgetCheckpoint,
    )
    .with_actor(GOAL_CONTROLLER_ACTOR)
    .with_payload(checkpoint)?;
    writer.append(&event).await
}

fn budget_checkpoints_path(state: &GoalState) -> std::path::PathBuf {
    state.state_dir.join(GOAL_BUDGET_CHECKPOINTS_FILE)
}
