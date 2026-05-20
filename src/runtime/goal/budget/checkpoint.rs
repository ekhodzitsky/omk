use anyhow::Result;
use chrono::{DateTime, Utc};

use crate::runtime::db::types::BudgetCheckpoint;
use crate::runtime::db::{global_db, BudgetRepo};
use crate::runtime::goal::state::{parse_goal_duration_secs, GoalState};

use super::events::append_budget_checkpoint_event;
use super::{collect_goal_budget_usage, remaining_tokens, remaining_usd, GoalBudgetCheckpoint};

pub(crate) async fn append_budget_checkpoint(
    state: &GoalState,
    label: &str,
) -> Result<GoalBudgetCheckpoint> {
    let checkpoint = build_budget_checkpoint(state, label, Utc::now()).await;
    if let Some(db) = global_db() {
        let record = goal_checkpoint_to_record(&checkpoint)?;
        db.budget_repo()
            .append_checkpoint(&record)
            .await
            .map_err(|e| anyhow::anyhow!("db error: {e}"))?;
    } else {
        // Fallback to JSONL when global DB is not initialized (tests, legacy paths).
        let line = serde_json::to_vec(&checkpoint)?;
        let mut content = line;
        content.push(b'\n');
        crate::runtime::atomic::atomic_append(&budget_checkpoints_path(state), &content).await?;
    }
    append_budget_checkpoint_event(state, &checkpoint).await?;
    Ok(checkpoint)
}

pub(super) async fn read_budget_checkpoints(
    state: &GoalState,
) -> Result<Vec<GoalBudgetCheckpoint>> {
    if let Some(db) = global_db() {
        let records = db
            .budget_repo()
            .get_by_goal(&state.goal_id)
            .await
            .map_err(|e| anyhow::anyhow!("db error: {e}"))?;
        return Ok(records.into_iter().map(record_to_goal_checkpoint).collect());
    }

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
                tracing::warn!(
                    line = line_no + 1,
                    error = %error,
                    "Skipping malformed goal budget checkpoint"
                );
            }
        }
    }
    Ok(checkpoints)
}

pub(super) async fn build_budget_checkpoint(
    state: &GoalState,
    label: &str,
    recorded_at: DateTime<Utc>,
) -> GoalBudgetCheckpoint {
    let total_budget_secs = state
        .budget_time
        .as_deref()
        .and_then(parse_goal_duration_secs);
    let elapsed_since_created_secs = u64::try_from(
        recorded_at
            .signed_duration_since(state.created_at)
            .num_seconds(),
    )
    .unwrap_or(0);
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

fn goal_checkpoint_to_record(cp: &GoalBudgetCheckpoint) -> Result<BudgetCheckpoint> {
    Ok(BudgetCheckpoint {
        checkpoint_id: None,
        goal_id: cp.goal_id.clone(),
        version: cp.version as i32,
        label: cp.label.clone(),
        status: cp.status.to_string(),
        phase: cp.phase.to_string(),
        recorded_at: cp.recorded_at.timestamp(),
        budget_time: cp.budget_time.clone(),
        total_budget_secs: cp.total_budget_secs.map(|v| v as i64),
        elapsed_since_created_secs: cp.elapsed_since_created_secs as i64,
        remaining_budget_secs: cp.remaining_budget_secs.map(|v| v as i64),
        budget_tokens: cp.budget_tokens.map(|v| v as i64),
        used_tokens: cp.used_tokens as i64,
        remaining_budget_tokens: cp.remaining_budget_tokens.map(|v| v as i64),
        budget_usd: cp.budget_usd.map(|v| (v * 100.0) as i64),
        estimated_cost_usd: (cp.estimated_cost_usd * 100.0) as i64,
        remaining_budget_usd: cp.remaining_budget_usd.map(|v| (v * 100.0) as i64),
        limit_value: None,
        used_value: None,
        created_at: cp.recorded_at.timestamp(),
    })
}

fn record_to_goal_checkpoint(record: BudgetCheckpoint) -> GoalBudgetCheckpoint {
    GoalBudgetCheckpoint {
        version: record.version as u32,
        goal_id: record.goal_id,
        label: record.label,
        status: parse_goal_status(&record.status)
            .unwrap_or(super::super::state::GoalStatus::NotReady),
        phase: parse_goal_phase(&record.phase).unwrap_or(super::super::state::GoalPhase::Intake),
        recorded_at: chrono::DateTime::from_timestamp(record.recorded_at, 0)
            .unwrap_or_else(chrono::Utc::now),
        budget_time: record.budget_time,
        total_budget_secs: record.total_budget_secs.map(|v| v as u64),
        elapsed_since_created_secs: record.elapsed_since_created_secs as u64,
        remaining_budget_secs: record.remaining_budget_secs.map(|v| v as u64),
        budget_tokens: record.budget_tokens.map(|v| v as u64),
        used_tokens: record.used_tokens as u64,
        remaining_budget_tokens: record.remaining_budget_tokens.map(|v| v as u64),
        budget_usd: record.budget_usd.map(|v| v as f64 / 100.0),
        estimated_cost_usd: record.estimated_cost_usd as f64 / 100.0,
        remaining_budget_usd: record.remaining_budget_usd.map(|v| v as f64 / 100.0),
    }
}

fn parse_goal_status(s: &str) -> Result<super::super::state::GoalStatus> {
    match s {
        "running" => Ok(super::super::state::GoalStatus::Running),
        "ready" => Ok(super::super::state::GoalStatus::Ready),
        "not_ready" => Ok(super::super::state::GoalStatus::NotReady),
        "blocked_on_human" => Ok(super::super::state::GoalStatus::BlockedOnHuman),
        "blocked_on_external" => Ok(super::super::state::GoalStatus::BlockedOnExternal),
        "needs_more_budget" => Ok(super::super::state::GoalStatus::NeedsMoreBudget),
        "failed_infra" => Ok(super::super::state::GoalStatus::FailedInfra),
        "paused" => Ok(super::super::state::GoalStatus::Paused),
        "cancelled" => Ok(super::super::state::GoalStatus::Cancelled),
        _ => anyhow::bail!("unknown goal status: {s}"),
    }
}

fn parse_goal_phase(s: &str) -> Result<super::super::state::GoalPhase> {
    match s {
        "intake" => Ok(super::super::state::GoalPhase::Intake),
        "planning" => Ok(super::super::state::GoalPhase::Planning),
        "decomposition" => Ok(super::super::state::GoalPhase::Decomposition),
        "execution" => Ok(super::super::state::GoalPhase::Execution),
        "verification_design" => Ok(super::super::state::GoalPhase::VerificationDesign),
        "proof" => Ok(super::super::state::GoalPhase::Proof),
        _ => anyhow::bail!("unknown goal phase: {s}"),
    }
}

fn budget_checkpoints_path(state: &GoalState) -> std::path::PathBuf {
    state
        .state_dir
        .join(crate::runtime::goal::state::GOAL_BUDGET_CHECKPOINTS_FILE)
}
