use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::runtime::goal::state::{GoalPhase, GoalStatus, GOAL_CONTROLLER_ACTOR};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalBudgetExhaustedEvent {
    pub action: String,
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
    pub budget_source: String,
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
pub struct GoalBudgetExtendedEvent {
    pub previous_budget_time: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub added_budget_time: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub added_budget_secs: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new_budget_time: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new_total_budget_secs: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_budget_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub added_budget_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new_budget_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_budget_usd: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub added_budget_usd: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new_budget_usd: Option<f64>,
    pub elapsed_since_created_secs: u64,
    pub used_tokens: u64,
    pub estimated_cost_usd: f64,
    pub status: GoalStatus,
    pub phase: GoalPhase,
    pub recorded_at: DateTime<Utc>,
}

pub async fn append_budget_extended_event(
    state: &crate::runtime::goal::state::GoalState,
    payload: &GoalBudgetExtendedEvent,
) -> Result<()> {
    let writer = crate::runtime::events::EventWriter::new(
        state.state_dir.join(crate::runtime::config::EVENTS_FILE),
    );
    let event = crate::runtime::events::Event::new(
        crate::runtime::events::RunId(state.goal_id.clone()),
        crate::runtime::events::EventKind::GoalBudgetExtended,
    )
    .with_actor(GOAL_CONTROLLER_ACTOR)
    .with_payload(payload)?;
    writer.append(&event).await
}

pub async fn append_budget_exhausted_event(
    state: &crate::runtime::goal::state::GoalState,
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

pub async fn append_budget_checkpoint_event(
    state: &crate::runtime::goal::state::GoalState,
    checkpoint: &super::GoalBudgetCheckpoint,
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
