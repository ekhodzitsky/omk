use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use uuid::Uuid;

use crate::runtime::events::{Event, EventBuilder, EventKind, EventWriter, RunId};

pub const GOALS_DIR: &str = "goals";
pub const GOAL_STATE_FILE: &str = "goal.json";
pub const GOAL_FAILURE_FILE: &str = "failure.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GoalStatus {
    Running,
    Ready,
    NotReady,
    BlockedOnHuman,
    BlockedOnExternal,
    NeedsMoreBudget,
    FailedInfra,
    Cancelled,
}

impl std::fmt::Display for GoalStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = match self {
            GoalStatus::Running => "running",
            GoalStatus::Ready => "ready",
            GoalStatus::NotReady => "not_ready",
            GoalStatus::BlockedOnHuman => "blocked_on_human",
            GoalStatus::BlockedOnExternal => "blocked_on_external",
            GoalStatus::NeedsMoreBudget => "needs_more_budget",
            GoalStatus::FailedInfra => "failed_infra",
            GoalStatus::Cancelled => "cancelled",
        };
        write!(f, "{value}")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalTerminalCriteria {
    pub proof_required: bool,
    pub gates_required: bool,
    pub human_blockers_stop: bool,
}

impl Default for GoalTerminalCriteria {
    fn default() -> Self {
        Self {
            proof_required: true,
            gates_required: true,
            human_blockers_stop: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalFailure {
    pub reason: String,
    pub recorded_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalState {
    #[serde(default = "default_goal_version")]
    pub version: u32,
    pub goal_id: String,
    pub original_goal: String,
    pub normalized_goal: String,
    pub status: GoalStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<DateTime<Utc>>,
    pub until_ready: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget_time: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_agents: Option<usize>,
    pub terminal_criteria: GoalTerminalCriteria,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure: Option<GoalFailure>,
    pub state_dir: PathBuf,
}

fn default_goal_version() -> u32 {
    1
}

#[derive(Debug, Clone)]
pub struct CreateGoalOptions {
    pub until_ready: bool,
    pub budget_time: Option<String>,
    pub max_agents: Option<usize>,
}

impl GoalState {
    pub fn state_file(&self) -> PathBuf {
        self.state_dir.join(GOAL_STATE_FILE)
    }

    pub async fn save(&self) -> Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        crate::runtime::atomic::atomic_write(&self.state_file(), json.as_bytes()).await
    }

    pub async fn load(goal_dir: &Path) -> Result<Self> {
        let path = goal_dir.join(GOAL_STATE_FILE);
        let json = tokio::fs::read_to_string(&path)
            .await
            .with_context(|| format!("Failed to read goal state: {}", path.display()))?;
        let state = serde_json::from_str(&json)
            .with_context(|| format!("Failed to parse goal state: {}", path.display()))?;
        Ok(state)
    }
}

pub fn goals_dir() -> PathBuf {
    crate::runtime::config::omk_state_dir().join(GOALS_DIR)
}

pub async fn create_goal(goal: &str, options: CreateGoalOptions) -> Result<GoalState> {
    let id = generate_goal_id();
    let goal_dir = goals_dir().join(&id);
    crate::runtime::config::ensure_private_dir(&goal_dir).await?;

    let now = Utc::now();
    let state = GoalState {
        version: 1,
        goal_id: id.clone(),
        original_goal: goal.to_string(),
        normalized_goal: normalize_goal(goal),
        status: GoalStatus::NotReady,
        created_at: now,
        updated_at: now,
        completed_at: Some(now),
        until_ready: options.until_ready,
        budget_time: options.budget_time,
        max_agents: options.max_agents,
        terminal_criteria: GoalTerminalCriteria::default(),
        failure: None,
        state_dir: goal_dir.clone(),
    };
    state.save().await?;

    let writer = EventWriter::new(goal_dir.join(crate::runtime::config::EVENTS_FILE));
    let builder = EventBuilder::new(RunId(id.clone()));
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    writer
        .append_many(&[
            builder.run_started("goal", &cwd, goal)?,
            builder.run_failed("goal runtime scaffold created without agent execution")?,
        ])
        .await?;

    Ok(state)
}

pub async fn list_goals() -> Result<Vec<GoalState>> {
    let dir = goals_dir();
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut entries = tokio::fs::read_dir(&dir).await?;
    let mut goals = Vec::new();
    while let Some(entry) = entries.next_entry().await? {
        if entry.file_type().await?.is_dir() {
            match GoalState::load(&entry.path()).await {
                Ok(state) => goals.push(state),
                Err(error) => tracing::warn!(
                    path = %entry.path().display(),
                    error = %error,
                    "Skipping unreadable goal state"
                ),
            }
        }
    }

    goals.sort_by(|a, b| {
        b.created_at
            .cmp(&a.created_at)
            .then_with(|| b.goal_id.cmp(&a.goal_id))
    });
    Ok(goals)
}

pub async fn resolve_goal(goal_id: &str) -> Result<GoalState> {
    if goal_id == "latest" {
        let mut goals = list_goals().await?;
        if let Some(goal) = goals.drain(..).next() {
            return Ok(goal);
        }
        anyhow::bail!("No goals found");
    }

    let goal_dir = goals_dir().join(goal_id);
    if !goal_dir.exists() {
        anyhow::bail!("Goal '{}' not found", goal_id);
    }
    GoalState::load(&goal_dir).await
}

pub async fn cancel_goal(goal_id: &str) -> Result<GoalState> {
    let mut state = resolve_goal(goal_id).await?;
    let now = Utc::now();
    state.status = GoalStatus::Cancelled;
    state.updated_at = now;
    state.completed_at = Some(now);
    state.failure = Some(GoalFailure {
        reason: "cancelled by user".to_string(),
        recorded_at: now,
    });
    state.save().await?;

    let failure_json = serde_json::to_string_pretty(&state)?;
    crate::runtime::atomic::atomic_write(
        &state.state_dir.join(GOAL_FAILURE_FILE),
        failure_json.as_bytes(),
    )
    .await?;

    let writer = EventWriter::new(state.state_dir.join(crate::runtime::config::EVENTS_FILE));
    let run_id = RunId(state.goal_id.clone());
    let interrupted = Event::new(run_id.clone(), EventKind::ManualInterrupt).with_actor("omk-cli");
    let failed = EventBuilder::new(run_id).run_failed("cancelled by user")?;
    writer.append_many(&[interrupted, failed]).await?;

    Ok(state)
}

fn generate_goal_id() -> String {
    let suffix = Uuid::new_v4().to_string();
    format!(
        "goal-{}-{}",
        Utc::now().format("%Y%m%d-%H%M%S-%3f"),
        &suffix[..8]
    )
}

fn normalize_goal(goal: &str) -> String {
    goal.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn goal_status_serializes_as_snake_case() {
        let value = serde_json::to_value(GoalStatus::NotReady).unwrap();
        assert_eq!(value, "not_ready");
    }

    #[test]
    fn normalize_goal_collapses_whitespace() {
        assert_eq!(normalize_goal("  ship   it\nwell  "), "ship it well");
    }
}
