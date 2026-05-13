use anyhow::Result;
use chrono::{DateTime, Utc};
use tracing::warn;

use crate::runtime::goal::state::{
    parse_goal_duration_secs, GoalState, GOAL_BUDGET_CHECKPOINTS_FILE,
};

use super::events::append_budget_checkpoint_event;
use super::{collect_goal_budget_usage, remaining_tokens, remaining_usd, GoalBudgetCheckpoint};

pub async fn append_budget_checkpoint(
    state: &GoalState,
    label: &str,
) -> Result<GoalBudgetCheckpoint> {
    let checkpoint = build_budget_checkpoint(state, label, Utc::now()).await;
    let line = serde_json::to_vec(&checkpoint)?;
    let mut content = line;
    content.push(b'\n');
    crate::runtime::atomic::atomic_append(&budget_checkpoints_path(state), &content).await?;
    append_budget_checkpoint_event(state, &checkpoint).await?;
    Ok(checkpoint)
}

pub async fn read_budget_checkpoints(state: &GoalState) -> Result<Vec<GoalBudgetCheckpoint>> {
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

pub async fn build_budget_checkpoint(
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
    let usage = collect_goal_budget_usage(state).await;

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
        budget_tokens: state.budget_tokens,
        used_tokens: usage.used_tokens,
        remaining_budget_tokens: remaining_tokens(state.budget_tokens, usage.used_tokens),
        budget_usd: state.budget_usd,
        estimated_cost_usd: usage.estimated_cost_usd,
        remaining_budget_usd: remaining_usd(state.budget_usd, usage.estimated_cost_usd),
    }
}

fn budget_checkpoints_path(state: &GoalState) -> std::path::PathBuf {
    state.state_dir.join(GOAL_BUDGET_CHECKPOINTS_FILE)
}
