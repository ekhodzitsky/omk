use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use tracing::warn;

use super::state::{
    format_goal_duration_secs, parse_goal_duration_secs, GoalPhase, GoalState, GoalStatus,
    GOAL_AGENT_RUNS_DIR, GOAL_ARTIFACTS_DIR, GOAL_BUDGET_CHECKPOINTS_FILE, GOAL_CONTROLLER_ACTOR,
};

const STANDARD_INPUT_USD_PER_1M_TOKENS: f64 = 2.0;
const STANDARD_OUTPUT_USD_PER_1M_TOKENS: f64 = 8.0;

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
}

#[derive(Debug, Clone, Default)]
pub struct GoalBudgetAdd {
    pub time: Option<String>,
    pub tokens: Option<u64>,
    pub usd: Option<f64>,
}

#[derive(Debug, Clone, Copy, Default)]
struct GoalBudgetUsage {
    used_tokens: u64,
    estimated_cost_usd: f64,
}

#[derive(Debug, Clone)]
struct GoalBudgetExhaustion {
    budget_source: &'static str,
    message_detail: String,
    remaining_budget_secs: Option<u64>,
    remaining_budget_tokens: Option<u64>,
    remaining_budget_usd: Option<f64>,
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
    budget_source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    budget_tokens: Option<u64>,
    used_tokens: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    remaining_budget_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    budget_usd: Option<f64>,
    estimated_cost_usd: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    remaining_budget_usd: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GoalBudgetExtendedEvent {
    previous_budget_time: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    added_budget_time: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    added_budget_secs: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    new_budget_time: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    new_total_budget_secs: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    previous_budget_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    added_budget_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    new_budget_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    previous_budget_usd: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    added_budget_usd: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    new_budget_usd: Option<f64>,
    elapsed_since_created_secs: u64,
    used_tokens: u64,
    estimated_cost_usd: f64,
    status: GoalStatus,
    phase: GoalPhase,
    recorded_at: DateTime<Utc>,
}

pub async fn goal_budget(goal_id: &str) -> Result<GoalBudgetReport> {
    let state = super::resolve_goal(goal_id).await?;
    let checkpoints = read_budget_checkpoints(&state).await?;
    let usage = collect_goal_budget_usage(&state);
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
    let elapsed_since_created_secs = now
        .signed_duration_since(state.created_at)
        .num_seconds()
        .max(0) as u64;
    let usage = collect_goal_budget_usage(&state);
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
    state.save().await?;

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
    let elapsed_since_created_secs = now
        .signed_duration_since(state.created_at)
        .num_seconds()
        .max(0) as u64;

    let usage = collect_goal_budget_usage(state);
    let Some(exhaustion) =
        first_budget_exhaustion(state, total_budget_secs, elapsed_since_created_secs, usage)
    else {
        return Ok(());
    };

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

async fn append_budget_extended_event(
    state: &GoalState,
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
    let usage = collect_goal_budget_usage(state);

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

fn collect_goal_budget_usage(state: &GoalState) -> GoalBudgetUsage {
    let root = state
        .state_dir
        .join(GOAL_ARTIFACTS_DIR)
        .join(GOAL_AGENT_RUNS_DIR);
    if !root.exists() {
        return GoalBudgetUsage::default();
    }

    let mut usage = GoalBudgetUsage::default();
    for entry in walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_file())
        .filter(|entry| entry.file_name() == "wire-events.jsonl")
    {
        usage.add(collect_wire_event_file_usage(entry.path()));
    }
    usage
}

fn collect_wire_event_file_usage(path: &Path) -> GoalBudgetUsage {
    let Ok(content) = std::fs::read_to_string(path) else {
        return GoalBudgetUsage::default();
    };

    let mut anonymous = GoalBudgetUsage::default();
    let mut by_message: HashMap<String, GoalBudgetUsage> = HashMap::new();
    for line in content.lines().filter(|line| !line.trim().is_empty()) {
        let Some((message_id, usage)) = parse_wire_status_usage(line) else {
            continue;
        };
        if let Some(message_id) = message_id {
            by_message
                .entry(message_id)
                .and_modify(|current| current.keep_max(usage))
                .or_insert(usage);
        } else {
            anonymous.add(usage);
        }
    }

    for usage in by_message.into_values() {
        anonymous.add(usage);
    }
    anonymous
}

fn parse_wire_status_usage(line: &str) -> Option<(Option<String>, GoalBudgetUsage)> {
    let value: serde_json::Value = serde_json::from_str(line).ok()?;
    let params = value.get("params")?;
    let event_type = params.get("type")?.as_str()?;
    if !event_type.eq_ignore_ascii_case("status_update")
        && !event_type.eq_ignore_ascii_case("status-update")
    {
        return None;
    }
    let payload = params.get("payload")?;
    let message_id = payload
        .get("message_id")
        .and_then(|value| value.as_str())
        .map(ToString::to_string);
    token_usage_from_payload(payload).map(|usage| (message_id, usage))
}

fn token_usage_from_payload(payload: &serde_json::Value) -> Option<GoalBudgetUsage> {
    let token_usage = payload.get("token_usage")?;
    if let Some(total) = token_usage.as_u64() {
        return Some(GoalBudgetUsage {
            used_tokens: total,
            estimated_cost_usd: estimate_unknown_token_cost(total),
        });
    }

    let input_other = token_usage
        .get("input_other")
        .and_then(|value| value.as_u64())
        .unwrap_or(0);
    let input_cache_read = token_usage
        .get("input_cache_read")
        .and_then(|value| value.as_u64())
        .unwrap_or(0);
    let input_cache_creation = token_usage
        .get("input_cache_creation")
        .and_then(|value| value.as_u64())
        .unwrap_or(0);
    let output = token_usage
        .get("output")
        .and_then(|value| value.as_u64())
        .unwrap_or(0);
    let input = input_other
        .saturating_add(input_cache_read)
        .saturating_add(input_cache_creation);
    let total = input.saturating_add(output);
    (total > 0).then(|| GoalBudgetUsage {
        used_tokens: total,
        estimated_cost_usd: estimate_split_token_cost(input, output),
    })
}

fn estimate_unknown_token_cost(tokens: u64) -> f64 {
    (tokens as f64 / 1_000_000.0) * STANDARD_OUTPUT_USD_PER_1M_TOKENS
}

fn estimate_split_token_cost(input_tokens: u64, output_tokens: u64) -> f64 {
    (input_tokens as f64 / 1_000_000.0) * STANDARD_INPUT_USD_PER_1M_TOKENS
        + (output_tokens as f64 / 1_000_000.0) * STANDARD_OUTPUT_USD_PER_1M_TOKENS
}

fn remaining_tokens(budget_tokens: Option<u64>, used_tokens: u64) -> Option<u64> {
    budget_tokens.map(|budget| budget.saturating_sub(used_tokens))
}

fn remaining_usd(budget_usd: Option<f64>, estimated_cost_usd: f64) -> Option<f64> {
    budget_usd.map(|budget| (budget - estimated_cost_usd).max(0.0))
}

impl GoalBudgetUsage {
    fn add(&mut self, other: GoalBudgetUsage) {
        self.used_tokens = self.used_tokens.saturating_add(other.used_tokens);
        self.estimated_cost_usd += other.estimated_cost_usd;
    }

    fn keep_max(&mut self, other: GoalBudgetUsage) {
        if other.used_tokens > self.used_tokens {
            *self = other;
        }
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
