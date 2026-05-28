use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::warn;

mod checkpoint;
mod events;
mod per_task;
mod usage;

pub use per_task::{evaluate_task_budget, PerTaskBudgetSnapshot};

pub(crate) use checkpoint::append_budget_checkpoint;
use checkpoint::read_budget_checkpoints;
use events::{
    append_budget_exhausted_event, append_budget_extended_event, GoalBudgetExhaustedEvent,
    GoalBudgetExtendedEvent,
};
use usage::{collect_goal_budget_usage, GoalBudgetUsage};

use super::state::{
    format_goal_duration_secs, parse_goal_duration_secs, FileSystemGoalStateStore, GoalPhase,
    GoalState, GoalStateStore, GoalStatus,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget_tokens: Option<u64>,
    pub used_tokens: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remaining_budget_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget_usd: Option<f64>,
    pub estimated_cost_usd: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remaining_budget_usd: Option<f64>,
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
    pub budget_tokens: Option<u64>,
    pub used_tokens: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remaining_budget_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget_usd: Option<f64>,
    pub estimated_cost_usd: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remaining_budget_usd: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest: Option<GoalBudgetCheckpoint>,
    pub checkpoints: Vec<GoalBudgetCheckpoint>,
    #[serde(default)]
    pub spent_usd: f64,
    #[serde(default)]
    pub spent_tokens: u64,
    #[serde(default)]
    pub spent_seconds: u64,
}

#[derive(Debug, Clone, Default)]
pub struct GoalBudgetAdd {
    pub time: Option<String>,
    pub tokens: Option<u64>,
    pub usd: Option<f64>,
}

#[derive(Debug, Clone)]
struct GoalBudgetExhaustion {
    budget_source: &'static str,
    message_detail: String,
    remaining_budget_secs: Option<u64>,
    remaining_budget_tokens: Option<u64>,
    remaining_budget_usd: Option<f64>,
}

pub async fn goal_budget(goal_id: &str) -> Result<GoalBudgetReport> {
    let state = super::resolve_goal(goal_id).await?;
    let checkpoints = read_budget_checkpoints(&state).await?;
    let usage = collect_goal_budget_usage(&state).await;

    let tracker = crate::cost::tracker::CostTracker::for_goal(
        &state.state_dir,
        state.cost_tracker_path.as_deref(),
    );
    let (spent_usd, spent_tokens, spent_seconds) = match tracker.load().await {
        Ok(costs) => {
            let spent_usd = costs.iter().map(|c| c.estimate.estimated_usd).sum();
            let spent_tokens = costs
                .iter()
                .map(|c| c.estimate.input_tokens + c.estimate.output_tokens)
                .sum();
            let spent_seconds = costs.iter().map(|c| c.estimate.duration_secs).sum();
            (spent_usd, spent_tokens, spent_seconds)
        }
        Err(_) => (0.0, 0, 0),
    };

    Ok(GoalBudgetReport {
        version: 1,
        goal_id: state.goal_id,
        generated_at: Utc::now(),
        budget_time: state.budget_time.clone(),
        total_budget_secs: state
            .budget_time
            .as_deref()
            .and_then(parse_goal_duration_secs),
        budget_tokens: state.budget_tokens,
        used_tokens: usage.used_tokens,
        remaining_budget_tokens: remaining_tokens(state.budget_tokens, usage.used_tokens),
        budget_usd: state.budget_usd,
        estimated_cost_usd: usage.estimated_cost_usd,
        remaining_budget_usd: remaining_usd(state.budget_usd, usage.estimated_cost_usd),
        latest: checkpoints.last().cloned(),
        checkpoints,
        spent_usd,
        spent_tokens,
        spent_seconds,
    })
}

pub async fn add_goal_budget(goal_id: &str, added_budget_time: &str) -> Result<GoalState> {
    add_goal_budget_limits(
        goal_id,
        GoalBudgetAdd {
            time: Some(added_budget_time.to_string()),
            tokens: None,
            usd: None,
        },
    )
    .await
}

pub async fn add_goal_budget_limits(goal_id: &str, add: GoalBudgetAdd) -> Result<GoalState> {
    if add.time.is_none() && add.tokens.is_none() && add.usd.is_none() {
        anyhow::bail!("Provide at least one budget extension: --time, --tokens, or --usd");
    }
    let mut state = super::resolve_goal(goal_id).await?;
    if matches!(state.status, GoalStatus::Ready | GoalStatus::Cancelled) {
        anyhow::bail!(
            "Goal '{}' is terminal ({}) and cannot receive more budget",
            state.goal_id,
            state.status
        );
    }
    let now = Utc::now();
    let elapsed_since_created_secs =
        u64::try_from(now.signed_duration_since(state.created_at).num_seconds()).unwrap_or(0);
    let usage = collect_goal_budget_usage(&state).await;
    let previous_budget_time = state.budget_time.clone();
    let previous_budget_tokens = state.budget_tokens;
    let previous_budget_usd = state.budget_usd;

    let mut added_budget_secs = None;
    let mut new_total_budget_secs = None;
    let mut new_budget_time = None;
    if let Some(added_budget_time) = add.time.as_deref() {
        let added_secs = parse_goal_duration_secs(added_budget_time)
            .filter(|secs| *secs > 0)
            .with_context(|| format!("Invalid budget duration: {added_budget_time}"))?;
        let current_total_budget_secs = state
            .budget_time
            .as_deref()
            .and_then(parse_goal_duration_secs)
            .unwrap_or(elapsed_since_created_secs);
        let new_total = current_total_budget_secs
            .max(elapsed_since_created_secs)
            .checked_add(added_secs)
            .context("Goal budget duration overflowed")?;
        let formatted = format_goal_duration_secs(new_total);
        state.budget_time = Some(formatted.clone());
        added_budget_secs = Some(added_secs);
        new_total_budget_secs = Some(new_total);
        new_budget_time = Some(formatted);
    }

    let mut new_budget_tokens = None;
    if let Some(added_tokens) = add.tokens {
        if added_tokens == 0 {
            anyhow::bail!("Invalid token budget extension: tokens must be greater than zero");
        }
        let current_budget_tokens = state.budget_tokens.unwrap_or(usage.used_tokens);
        let new_total = current_budget_tokens
            .max(usage.used_tokens)
            .checked_add(added_tokens)
            .context("Goal token budget overflowed")?;
        state.budget_tokens = Some(new_total);
        new_budget_tokens = Some(new_total);
    }

    let mut new_budget_usd = None;
    if let Some(added_usd) = add.usd {
        if !added_usd.is_finite() || added_usd <= 0.0 {
            anyhow::bail!("Invalid USD budget extension: usd must be greater than zero");
        }
        let current_budget_usd = state.budget_usd.unwrap_or(usage.estimated_cost_usd);
        let new_total = current_budget_usd.max(usage.estimated_cost_usd) + added_usd;
        state.budget_usd = Some(new_total);
        new_budget_usd = Some(new_total);
    }

    if state.status == GoalStatus::NeedsMoreBudget {
        state.status = GoalStatus::NotReady;
        state.completed_at = None;
    }
    state.updated_at = now;
    FileSystemGoalStateStore::new().save(&state).await?;

    append_budget_extended_event(
        &state,
        &GoalBudgetExtendedEvent {
            previous_budget_time,
            added_budget_time: add.time,
            added_budget_secs,
            new_budget_time,
            new_total_budget_secs,
            previous_budget_tokens,
            added_budget_tokens: add.tokens,
            new_budget_tokens,
            previous_budget_usd,
            added_budget_usd: add.usd,
            new_budget_usd,
            elapsed_since_created_secs,
            used_tokens: usage.used_tokens,
            estimated_cost_usd: usage.estimated_cost_usd,
            status: state.status,
            phase: state.phase,
            recorded_at: now,
        },
    )
    .await?;
    append_budget_checkpoint(&state, "budget_extended").await?;
    Ok(state)
}

pub(crate) async fn ensure_budget_available(state: &mut GoalState, action: &str) -> Result<()> {
    let now = Utc::now();
    let total_budget_secs = state
        .budget_time
        .as_deref()
        .and_then(parse_goal_duration_secs);
    let elapsed_since_created_secs =
        u64::try_from(now.signed_duration_since(state.created_at).num_seconds()).unwrap_or(0);

    let usage = collect_goal_budget_usage(state).await;

    let tracker = crate::cost::tracker::CostTracker::for_goal(
        &state.state_dir,
        state.cost_tracker_path.as_deref(),
    );
    let estimate = crate::cost::estimator::CostEstimate::from_budget(
        usage.used_tokens,
        elapsed_since_created_secs,
        usage.estimated_cost_usd,
    );
    if let Err(e) = tracker.record_budget_check(action, estimate).await {
        warn!(error = %e, "Failed to record budget cost");
    }

    let Some(exhaustion) =
        first_budget_exhaustion(state, total_budget_secs, elapsed_since_created_secs, usage)
    else {
        return Ok(());
    };

    state.status = GoalStatus::NeedsMoreBudget;
    state.updated_at = now;
    state.completed_at = Some(now);
    FileSystemGoalStateStore::new().save(state).await?;
    append_budget_exhausted_event(
        state,
        &GoalBudgetExhaustedEvent {
            action: action.to_string(),
            status: state.status,
            phase: state.phase,
            recorded_at: now,
            budget_source: exhaustion.budget_source.to_string(),
            budget_time: state.budget_time.clone(),
            total_budget_secs,
            elapsed_since_created_secs,
            remaining_budget_secs: exhaustion.remaining_budget_secs,
            budget_tokens: state.budget_tokens,
            used_tokens: usage.used_tokens,
            remaining_budget_tokens: exhaustion.remaining_budget_tokens,
            budget_usd: state.budget_usd,
            estimated_cost_usd: usage.estimated_cost_usd,
            remaining_budget_usd: exhaustion.remaining_budget_usd,
        },
    )
    .await?;
    append_budget_checkpoint(state, "budget_exhausted").await?;
    bail!(
        "Goal '{}' needs more budget before running `{}` ({} exhausted: {})",
        state.goal_id,
        action,
        exhaustion.budget_source,
        exhaustion.message_detail
    );
}

fn first_budget_exhaustion(
    state: &GoalState,
    total_budget_secs: Option<u64>,
    elapsed_since_created_secs: u64,
    usage: GoalBudgetUsage,
) -> Option<GoalBudgetExhaustion> {
    if let Some(total_budget_secs) = total_budget_secs {
        if elapsed_since_created_secs >= total_budget_secs {
            return Some(GoalBudgetExhaustion {
                budget_source: "time",
                message_detail: format!(
                    "budget_time={}, elapsed={}s",
                    state.budget_time.as_deref().unwrap_or("unbounded"),
                    elapsed_since_created_secs
                ),
                remaining_budget_secs: Some(0),
                remaining_budget_tokens: remaining_tokens(state.budget_tokens, usage.used_tokens),
                remaining_budget_usd: remaining_usd(state.budget_usd, usage.estimated_cost_usd),
            });
        }
    }

    if let Some(budget_tokens) = state.budget_tokens {
        if usage.used_tokens >= budget_tokens {
            return Some(GoalBudgetExhaustion {
                budget_source: "tokens",
                message_detail: format!(
                    "budget_tokens={}, used_tokens={}",
                    budget_tokens, usage.used_tokens
                ),
                remaining_budget_secs: total_budget_secs
                    .map(|total| total.saturating_sub(elapsed_since_created_secs)),
                remaining_budget_tokens: Some(0),
                remaining_budget_usd: remaining_usd(state.budget_usd, usage.estimated_cost_usd),
            });
        }
    }

    if let Some(budget_usd) = state.budget_usd {
        if usage.estimated_cost_usd >= budget_usd {
            return Some(GoalBudgetExhaustion {
                budget_source: "cost",
                message_detail: format!(
                    "budget_usd={:.6}, estimated_cost_usd={:.6}",
                    budget_usd, usage.estimated_cost_usd
                ),
                remaining_budget_secs: total_budget_secs
                    .map(|total| total.saturating_sub(elapsed_since_created_secs)),
                remaining_budget_tokens: remaining_tokens(state.budget_tokens, usage.used_tokens),
                remaining_budget_usd: Some(0.0),
            });
        }
    }

    None
}

fn remaining_tokens(budget_tokens: Option<u64>, used_tokens: u64) -> Option<u64> {
    budget_tokens.map(|budget| budget.saturating_sub(used_tokens))
}

fn remaining_usd(budget_usd: Option<f64>, estimated_cost_usd: f64) -> Option<f64> {
    budget_usd.map(|budget| (budget - estimated_cost_usd).max(0.0))
}
