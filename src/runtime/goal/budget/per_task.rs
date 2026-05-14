use chrono::Utc;
use serde::Serialize;

use super::usage::collect_goal_budget_usage;
use crate::runtime::goal::agent::GoalAgentTaskProposal;
use crate::runtime::goal::state::{parse_goal_duration_secs, GoalState};

#[derive(Debug, Clone, Serialize)]
pub struct PerTaskBudgetSnapshot {
    pub budget_time: Option<String>,
    pub total_budget_secs: Option<u64>,
    pub elapsed_since_created_secs: u64,
    pub remaining_budget_secs: Option<u64>,
    pub budget_tokens: Option<u64>,
    pub used_tokens: u64,
    pub remaining_budget_tokens: Option<u64>,
    pub budget_usd: Option<f64>,
    pub estimated_cost_usd: f64,
    pub remaining_budget_usd: Option<f64>,
    pub task_budget_secs: u64,
}

pub async fn evaluate_task_budget(
    state: &GoalState,
    proposal: &GoalAgentTaskProposal,
) -> Result<PerTaskBudgetSnapshot, String> {
    let now = Utc::now();
    let total_budget_secs = state
        .budget_time
        .as_deref()
        .and_then(parse_goal_duration_secs);
    let elapsed_since_created_secs = now
        .signed_duration_since(state.created_at)
        .num_seconds()
        .max(0) as u64;
    let usage = collect_goal_budget_usage(state).await;

    if let Some(total_budget_secs) = total_budget_secs {
        if elapsed_since_created_secs.saturating_add(proposal.budget_secs) > total_budget_secs {
            return Err(format!(
                "task budget_secs={} would exceed goal time budget: elapsed={}s, total={}s",
                proposal.budget_secs, elapsed_since_created_secs, total_budget_secs
            ));
        }
    }

    if let Some(budget_tokens) = state.budget_tokens {
        if usage.used_tokens >= budget_tokens {
            return Err(format!(
                "goal token budget exhausted: budget_tokens={}, used_tokens={}",
                budget_tokens, usage.used_tokens
            ));
        }
    }

    if let Some(budget_usd) = state.budget_usd {
        if usage.estimated_cost_usd >= budget_usd {
            return Err(format!(
                "goal cost budget exhausted: budget_usd={:.6}, estimated_cost_usd={:.6}",
                budget_usd, usage.estimated_cost_usd
            ));
        }
    }

    Ok(PerTaskBudgetSnapshot {
        budget_time: state.budget_time.clone(),
        total_budget_secs,
        elapsed_since_created_secs,
        remaining_budget_secs: total_budget_secs
            .map(|t| t.saturating_sub(elapsed_since_created_secs)),
        budget_tokens: state.budget_tokens,
        used_tokens: usage.used_tokens,
        remaining_budget_tokens: state
            .budget_tokens
            .map(|b| b.saturating_sub(usage.used_tokens)),
        budget_usd: state.budget_usd,
        estimated_cost_usd: usage.estimated_cost_usd,
        remaining_budget_usd: state
            .budget_usd
            .map(|b| (b - usage.estimated_cost_usd).max(0.0)),
        task_budget_secs: proposal.budget_secs,
    })
}
