use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::runtime::db::{
    handle::DbHandle, repo::goal::GoalRepo, types::GoalRecord, DbError,
};

use super::store::GoalStateStore;
use super::types::GoalState;

/// SQLite-backed implementation of [`GoalStateStore`].
///
/// Persists goal state to a single SQLite database file rather than
/// per-goal JSON files. The `state_dir` field is still materialised on
/// load so that downstream code can locate artifact directories.
#[derive(Debug, Clone)]
pub struct DbGoalStateStore {
    db: DbHandle,
}

impl DbGoalStateStore {
    pub fn new(db: DbHandle) -> Self {
        Self { db }
    }
}

impl GoalStateStore for DbGoalStateStore {
    async fn save(&self, state: &GoalState) -> Result<()> {
        let record = goal_state_to_record(state)?;
        self.db.goal_repo().create(&record).await.map_err(map_db_err)
    }

    async fn load(&self, goal_dir: &Path) -> Result<GoalState> {
        let goal_id = goal_dir
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        if goal_id.is_empty() {
            anyhow::bail!("goal_dir has no valid goal_id: {}", goal_dir.display());
        }

        let record = self
            .db
            .goal_repo()
            .get(&goal_id)
            .await
            .map_err(map_db_err)?
            .ok_or_else(|| super::GoalStateError::MissingFile {
                path: goal_dir.display().to_string(),
            })?;

        let mut state = record_to_goal_state(record)?;
        state.state_dir = goal_dir.to_path_buf();
        Ok(state)
    }

    async fn list(&self) -> Result<Vec<GoalState>> {
        let summaries = self
            .db
            .goal_repo()
            .list(crate::runtime::db::types::GoalFilter::default())
            .await
            .map_err(map_db_err)?;

        let mut goals = Vec::new();
        for summary in summaries {
            let goal_dir = super::persistence::goals_dir().join(&summary.goal_id);
            match self.load(&goal_dir).await {
                Ok(state) => goals.push(state),
                Err(error) => {
                    tracing::warn!(
                        goal_id = %summary.goal_id,
                        error = %error,
                        "Skipping unreadable goal state"
                    );
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
}

fn map_db_err(e: DbError) -> anyhow::Error {
    anyhow::anyhow!("db error: {e}")
}

fn goal_state_to_record(state: &GoalState) -> Result<GoalRecord> {
    Ok(GoalRecord {
        goal_id: state.goal_id.clone(),
        status: state.status.to_string(),
        phase: state.phase.to_string(),
        kind: None,
        original_goal: state.original_goal.clone(),
        normalized_goal: state.normalized_goal.clone(),
        goal_text: state.original_goal.clone(),
        project_dir: state.state_dir.display().to_string(),
        state_dir: state.state_dir.display().to_string(),
        policy: "local".to_string(),
        delivery_policy: state.delivery_policy.as_str().to_string(),
        merge_policy: state.merge_policy.as_str().to_string(),
        until_ready: state.until_ready,
        slice_execution: state.slice_execution,
        max_agents: state.max_agents.map(|v| v as i32),
        budget_time: state.budget_time.clone(),
        budget_tokens: state.budget_tokens.map(|v| v as i64),
        budget_usd: state.budget_usd.map(|v| (v * 100.0) as i64),
        cost_tracker_path: state.cost_tracker_path.as_ref().map(|p| p.display().to_string()),
        terminal_criteria: Some(serde_json::to_string(&state.terminal_criteria)?),
        failure: state
            .failure
            .as_ref()
            .map(|f| serde_json::to_string(f))
            .transpose()?,
        created_at: state.created_at.timestamp(),
        updated_at: state.updated_at.timestamp(),
        completed_at: state.completed_at.map(|dt| dt.timestamp()),
        controller_pid: None,
        version: state.version as i32,
    })
}

fn record_to_goal_state(record: GoalRecord) -> Result<GoalState> {
    let status = parse_goal_status(&record.status)?;
    let phase = parse_goal_phase(&record.phase)?;
    let delivery_policy = parse_delivery_policy(&record.delivery_policy)?;
    let merge_policy = parse_merge_policy(&record.merge_policy)?;

    let terminal_criteria = record
        .terminal_criteria
        .map(|s| serde_json::from_str(&s))
        .transpose()
        .unwrap_or_else(|e| {
            tracing::warn!(error = %e, "failed to parse terminal_criteria, using default");
            Some(Default::default())
        })
        .unwrap_or_default();

    let failure = record
        .failure
        .map(|s| serde_json::from_str(&s))
        .transpose()?;

    Ok(GoalState {
        version: record.version as u32,
        goal_id: record.goal_id,
        original_goal: record.original_goal,
        normalized_goal: record.normalized_goal,
        status,
        phase,
        created_at: chrono::DateTime::from_timestamp(record.created_at, 0)
            .unwrap_or_else(|| chrono::Utc::now()),
        updated_at: chrono::DateTime::from_timestamp(record.updated_at, 0)
            .unwrap_or_else(|| chrono::Utc::now()),
        completed_at: record.completed_at.and_then(|ts| chrono::DateTime::from_timestamp(ts, 0)),
        until_ready: record.until_ready,
        budget_time: record.budget_time,
        budget_tokens: record.budget_tokens.map(|v| v as u64),
        budget_usd: record.budget_usd.map(|v| v as f64 / 100.0),
        max_agents: record.max_agents.map(|v| v as usize),
        cost_tracker_path: record.cost_tracker_path.map(PathBuf::from),
        terminal_criteria,
        delivery_policy,
        merge_policy,
        slice_execution: record.slice_execution,
        artifacts: Vec::new(),
        failure,
        state_dir: PathBuf::from(record.state_dir),
    })
}

fn parse_goal_status(s: &str) -> Result<super::types::GoalStatus> {
    match s {
        "running" => Ok(super::types::GoalStatus::Running),
        "ready" => Ok(super::types::GoalStatus::Ready),
        "not_ready" => Ok(super::types::GoalStatus::NotReady),
        "blocked_on_human" => Ok(super::types::GoalStatus::BlockedOnHuman),
        "blocked_on_external" => Ok(super::types::GoalStatus::BlockedOnExternal),
        "needs_more_budget" => Ok(super::types::GoalStatus::NeedsMoreBudget),
        "failed_infra" => Ok(super::types::GoalStatus::FailedInfra),
        "paused" => Ok(super::types::GoalStatus::Paused),
        "cancelled" => Ok(super::types::GoalStatus::Cancelled),
        _ => anyhow::bail!("unknown goal status: {s}"),
    }
}

fn parse_goal_phase(s: &str) -> Result<super::types::GoalPhase> {
    match s {
        "intake" => Ok(super::types::GoalPhase::Intake),
        "planning" => Ok(super::types::GoalPhase::Planning),
        "decomposition" => Ok(super::types::GoalPhase::Decomposition),
        "execution" => Ok(super::types::GoalPhase::Execution),
        "verification_design" => Ok(super::types::GoalPhase::VerificationDesign),
        "proof" => Ok(super::types::GoalPhase::Proof),
        _ => anyhow::bail!("unknown goal phase: {s}"),
    }
}

fn parse_delivery_policy(s: &str) -> Result<super::super::GoalDeliveryPolicy> {
    match s {
        "local" => Ok(super::super::GoalDeliveryPolicy::Local),
        "draft_pr" => Ok(super::super::GoalDeliveryPolicy::DraftPr),
        "auto_pr" => Ok(super::super::GoalDeliveryPolicy::AutoPr),
        _ => anyhow::bail!("unknown delivery policy: {s}"),
    }
}

fn parse_merge_policy(s: &str) -> Result<super::super::GoalMergePolicy> {
    match s {
        "disabled" => Ok(super::super::GoalMergePolicy::Disabled),
        "manual" => Ok(super::super::GoalMergePolicy::Manual),
        "gated" => Ok(super::super::GoalMergePolicy::Gated),
        _ => anyhow::bail!("unknown merge policy: {s}"),
    }
}
