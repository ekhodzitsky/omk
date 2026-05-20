use std::path::Path;

use anyhow::Result;

use crate::runtime::db::{
    error::DbError,
    repo::{artifact::ArtifactRepo, goal::GoalRepo},
    types::{GoalFilter, GoalRecord},
    DbHandle,
};

use super::types::{GoalArtifact, GoalPhase, GoalState, GoalStatus, GoalTerminalCriteria};
use super::{
    duration::{format_goal_duration_secs, parse_goal_duration_secs},
    error::GoalStateError,
    store::GoalStateStore,
};

/// Path to the central SQLite database for all goal state.
pub fn goals_db_path() -> std::path::PathBuf {
    crate::runtime::config::omk_state_dir().join("omk.db")
}

/// Production implementation backed by SQLite, with JSON fallback for
/// backward compatibility.
///
/// Primary storage is SQLite (transactional, queryable).  Every `save`
/// also writes the JSON backup so that existing tooling that reads the
/// filesystem directly continues to work during the transition.
#[derive(Debug, Clone)]
pub struct DbGoalStateStore {
    db: DbHandle,
}

impl DbGoalStateStore {
    pub async fn open() -> Result<Self, DbError> {
        let db = DbHandle::open(goals_db_path()).await?;
        Ok(Self { db })
    }

    /// Open with an explicit handle (useful in tests).
    #[allow(dead_code)]
    pub fn with_handle(db: DbHandle) -> Self {
        Self { db }
    }
}

impl GoalStateStore for DbGoalStateStore {
    async fn save(&self, state: &GoalState) -> Result<()> {
        let record = goal_state_to_record(state);
        self.db.goal_repo().upsert(&record).await.map_err(db_to_anyhow)?;

        // Sync artifacts to DB (upsert by delete + recreate for simplicity).
        let artifact_repo = self.db.artifact_repo();
        artifact_repo.delete_by_goal(&state.goal_id).await.map_err(db_to_anyhow)?;
        for artifact in &state.artifacts {
            artifact_repo
                .register(&state.goal_id, &artifact.kind, &artifact.path.to_string_lossy(), None)
                .await
                .map_err(db_to_anyhow)?;
        }

        // Also write JSON backup for backward compatibility.
        json_backup_save(state).await?;
        Ok(())
    }

    async fn load(&self, goal_dir: &Path) -> Result<GoalState> {
        // Determine goal_id from the directory name.
        let goal_id = goal_dir
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| anyhow::anyhow!("invalid goal_dir: no directory name"))?;

        match self.db.goal_repo().get(goal_id).await {
            Ok(Some(record)) => {
                let mut state = goal_record_to_state(record)?;
                // Hydrate artifacts from DB.
                let artifacts = self
                    .db
                    .artifact_repo()
                    .get_by_goal(goal_id, None)
                    .await
                    .map_err(db_to_anyhow)?;
                state.artifacts = artifacts
                    .into_iter()
                    .map(|a| GoalArtifact {
                        kind: a.kind,
                        path: a.path.into(),
                        created_at: chrono::DateTime::from_timestamp(a.created_at, 0)
                            .unwrap_or_else(chrono::Utc::now),
                    })
                    .collect();
                state.state_dir = goal_dir.to_path_buf();
                Ok(state)
            }
            Ok(None) => {
                // Fallback to JSON for goals created before DB integration.
                json_backup_load(goal_dir).await
            }
            Err(e) => {
                tracing::warn!(error = %e, goal_id, "DB load failed; falling back to JSON");
                json_backup_load(goal_dir).await
            }
        }
    }

    async fn list(&self) -> Result<Vec<GoalState>> {
        let summaries = self
            .db
            .goal_repo()
            .list(GoalFilter::default())
            .await
            .map_err(db_to_anyhow)?;

        let mut goals = Vec::with_capacity(summaries.len());
        for summary in summaries {
            let goal_dir = super::persistence::goals_dir().join(&summary.goal_id);
            match self.load(&goal_dir).await {
                Ok(state) => goals.push(state),
                Err(e) => {
                    tracing::warn!(
                        goal_id = %summary.goal_id,
                        error = %e,
                        "Skipping unreadable goal state during list"
                    );
                }
            }
        }
        Ok(goals)
    }
}

// ---------------------------------------------------------------------------
// JSON backup helpers (preserves backward compatibility)
// ---------------------------------------------------------------------------

pub(crate) async fn json_backup_save(state: &GoalState) -> Result<()> {
    let path = state.state_dir.join(super::constants::GOAL_STATE_FILE);
    let json = serde_json::to_string_pretty(state)?;
    crate::runtime::atomic::atomic_write(&path, json.as_bytes()).await
}

pub(crate) async fn json_backup_load(goal_dir: &Path) -> Result<GoalState> {
    let path = goal_dir.join(super::constants::GOAL_STATE_FILE);
    let json = tokio::fs::read_to_string(&path).await.map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            GoalStateError::MissingFile {
                path: path.display().to_string(),
            }
        } else {
            GoalStateError::IoError {
                path: path.display().to_string(),
                reason: e.to_string(),
            }
        }
    })?;
    let mut state: GoalState =
        serde_json::from_str(&json).map_err(|e| GoalStateError::InvalidFormat {
            path: path.display().to_string(),
            reason: e.to_string(),
        })?;
    state.state_dir = goal_dir.to_path_buf();
    Ok(state)
}

// ---------------------------------------------------------------------------
// Conversions
// ---------------------------------------------------------------------------

fn goal_state_to_record(state: &GoalState) -> GoalRecord {
    GoalRecord {
        goal_id: state.goal_id.clone(),
        status: state.status.to_string(),
        phase: state.phase.to_string(),
        kind: None, // GoalKind is not stored on GoalState directly today.
        original_goal: state.original_goal.clone(),
        normalized_goal: state.normalized_goal.clone(),
        goal_text: state.original_goal.clone(),
        project_dir: state.state_dir.parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_default(),
        state_dir: state.state_dir.to_string_lossy().to_string(),
        policy: state.delivery_policy.as_str().to_string(),
        delivery_policy: state.delivery_policy.as_str().to_string(),
        merge_policy: state.merge_policy.as_str().to_string(),
        until_ready: state.until_ready,
        slice_execution: state.slice_execution,
        max_agents: state.max_agents.map(|m| m as i32),
        budget_time_secs: state
            .budget_time
            .as_deref()
            .and_then(parse_goal_duration_secs)
            .map(|s| s as i64),
        budget_tokens: state.budget_tokens.map(|t| t as i64),
        budget_usd: state.budget_usd.map(|u| (u * 100.0).round() as i64),
        cost_tracker_path: state.cost_tracker_path.as_ref().map(|p| p.to_string_lossy().to_string()),
        terminal_criteria: serde_json::to_string(&state.terminal_criteria).ok(),
        failure: state.failure.as_ref().and_then(|f| serde_json::to_string(f).ok()),
        created_at: state.created_at.timestamp(),
        updated_at: state.updated_at.timestamp(),
        completed_at: state.completed_at.map(|dt| dt.timestamp()),
        controller_pid: None,
        version: state.version as i32,
    }
}

fn goal_record_to_state(record: GoalRecord) -> Result<GoalState> {
    let status = parse_status(&record.status)?;
    let phase = parse_phase(&record.phase)?;
    let budget_time = record
        .budget_time_secs
        .map(|secs| format_goal_duration_secs(secs as u64));

    let terminal_criteria = match record.terminal_criteria.as_deref() {
        Some(json) => serde_json::from_str(json)
            .unwrap_or_else(|e| {
                tracing::warn!(error = %e, "Failed to parse terminal_criteria from DB");
                GoalTerminalCriteria::default()
            }),
        None => GoalTerminalCriteria::default(),
    };

    let failure = match record.failure.as_deref() {
        Some(json) => serde_json::from_str(json).ok(),
        None => None,
    };

    Ok(GoalState {
        version: record.version as u32,
        goal_id: record.goal_id,
        original_goal: record.original_goal,
        normalized_goal: record.normalized_goal,
        status,
        phase,
        created_at: chrono::DateTime::from_timestamp(record.created_at, 0)
            .unwrap_or_else(chrono::Utc::now),
        updated_at: chrono::DateTime::from_timestamp(record.updated_at, 0)
            .unwrap_or_else(chrono::Utc::now),
        completed_at: record.completed_at.and_then(|ts| chrono::DateTime::from_timestamp(ts, 0)),
        until_ready: record.until_ready,
        budget_time,
        budget_tokens: record.budget_tokens.map(|t| t as u64),
        budget_usd: record.budget_usd.map(|c| c as f64 / 100.0),
        max_agents: record.max_agents.map(|m| m as usize),
        cost_tracker_path: record.cost_tracker_path.map(std::path::PathBuf::from),
        terminal_criteria,
        delivery_policy: parse_delivery_policy(&record.delivery_policy),
        merge_policy: parse_merge_policy(&record.merge_policy),
        slice_execution: record.slice_execution,
        artifacts: Vec::new(), // hydrated separately
        failure,
        state_dir: Path::new(&record.state_dir).to_path_buf(),
    })
}

fn parse_status(s: &str) -> Result<GoalStatus> {
    match s {
        "running" => Ok(GoalStatus::Running),
        "ready" => Ok(GoalStatus::Ready),
        "not_ready" => Ok(GoalStatus::NotReady),
        "blocked_on_human" => Ok(GoalStatus::BlockedOnHuman),
        "blocked_on_external" => Ok(GoalStatus::BlockedOnExternal),
        "needs_more_budget" => Ok(GoalStatus::NeedsMoreBudget),
        "failed_infra" => Ok(GoalStatus::FailedInfra),
        "paused" => Ok(GoalStatus::Paused),
        "cancelled" => Ok(GoalStatus::Cancelled),
        _ => anyhow::bail!("unknown goal status: {s}"),
    }
}

fn parse_phase(s: &str) -> Result<GoalPhase> {
    match s {
        "intake" => Ok(GoalPhase::Intake),
        "planning" => Ok(GoalPhase::Planning),
        "decomposition" => Ok(GoalPhase::Decomposition),
        "execution" => Ok(GoalPhase::Execution),
        "verification_design" => Ok(GoalPhase::VerificationDesign),
        "proof" => Ok(GoalPhase::Proof),
        _ => anyhow::bail!("unknown goal phase: {s}"),
    }
}

fn parse_delivery_policy(s: &str) -> crate::runtime::goal::GoalDeliveryPolicy {
    match s {
        "draft-pr" => crate::runtime::goal::GoalDeliveryPolicy::DraftPr,
        "auto-pr" => crate::runtime::goal::GoalDeliveryPolicy::AutoPr,
        _ => crate::runtime::goal::GoalDeliveryPolicy::Local,
    }
}

fn parse_merge_policy(s: &str) -> crate::runtime::goal::GoalMergePolicy {
    match s {
        "manual" => crate::runtime::goal::GoalMergePolicy::Manual,
        "gated" => crate::runtime::goal::GoalMergePolicy::Gated,
        _ => crate::runtime::goal::GoalMergePolicy::Disabled,
    }
}

fn db_to_anyhow(e: DbError) -> anyhow::Error {
    anyhow::anyhow!("db error: {e}")
}
