use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use thiserror::Error;

use crate::runtime::goal::budget::GoalBudgetReport;
use crate::runtime::goal::proof::GoalProof;
use crate::runtime::goal::task_graph::GoalTaskGraph;

/// Error type for checkpoint operations.
#[derive(Error, Debug)]
pub enum RecoveryCheckpointError {
    #[error("serialization failed")]
    Serialization(#[source] serde_json::Error),
    #[error("deserialization failed")]
    Deserialization(#[source] serde_json::Error),
    #[error("io failed")]
    Io(#[source] std::io::Error),
    #[error("checkpoint not found: {0}")]
    NotFound(u32),
}

/// A snapshot of goal state before recovery execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryCheckpoint {
    pub checkpoint_id: u32,
    pub goal_id: String,
    pub git_commit: String,
    pub proof_snapshot: GoalProof,
    pub task_graph_snapshot: String,
    pub budget_snapshot: GoalBudgetReport,
    pub created_at: DateTime<Utc>,
}

impl RecoveryCheckpoint {
    /// Save checkpoint to disk as JSON.
    pub async fn save(&self, checkpoints_dir: &Path) -> Result<(), RecoveryCheckpointError> {
        let path = checkpoint_path(checkpoints_dir, self.checkpoint_id);
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(RecoveryCheckpointError::Io)?;
        }
        let json =
            serde_json::to_string_pretty(self).map_err(RecoveryCheckpointError::Serialization)?;
        tokio::fs::write(&path, json)
            .await
            .map_err(RecoveryCheckpointError::Io)?;
        Ok(())
    }

    /// Load checkpoint from disk by ID.
    pub async fn load(
        checkpoints_dir: &Path,
        checkpoint_id: u32,
    ) -> Result<Self, RecoveryCheckpointError> {
        let path = checkpoint_path(checkpoints_dir, checkpoint_id);
        if !path.exists() {
            return Err(RecoveryCheckpointError::NotFound(checkpoint_id));
        }
        let json = tokio::fs::read_to_string(&path)
            .await
            .map_err(RecoveryCheckpointError::Io)?;
        let checkpoint: Self =
            serde_json::from_str(&json).map_err(RecoveryCheckpointError::Deserialization)?;
        Ok(checkpoint)
    }

    /// Create a checkpoint from current goal state.
    pub fn from_state(
        checkpoint_id: u32,
        goal_id: String,
        git_commit: String,
        proof: GoalProof,
        task_graph: &GoalTaskGraph,
        budget: GoalBudgetReport,
    ) -> Result<Self, RecoveryCheckpointError> {
        let task_graph_snapshot =
            serde_json::to_string(task_graph).map_err(RecoveryCheckpointError::Serialization)?;
        Ok(Self {
            checkpoint_id,
            goal_id,
            git_commit,
            proof_snapshot: proof,
            task_graph_snapshot,
            budget_snapshot: budget,
            created_at: Utc::now(),
        })
    }

    /// Deserialize the task graph snapshot.
    pub fn task_graph(&self) -> Result<GoalTaskGraph, RecoveryCheckpointError> {
        serde_json::from_str(&self.task_graph_snapshot)
            .map_err(RecoveryCheckpointError::Deserialization)
    }
}

fn checkpoint_path(checkpoints_dir: &Path, checkpoint_id: u32) -> PathBuf {
    checkpoints_dir.join(format!("checkpoint_{}.json", checkpoint_id))
}

/// List all checkpoint IDs in a directory.
pub async fn list_checkpoints(checkpoints_dir: &Path) -> Result<Vec<u32>, RecoveryCheckpointError> {
    let mut entries = tokio::fs::read_dir(checkpoints_dir)
        .await
        .map_err(RecoveryCheckpointError::Io)?;
    let mut ids = Vec::new();
    while let Some(entry) = entries
        .next_entry()
        .await
        .map_err(RecoveryCheckpointError::Io)?
    {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if let Some(stem) = name.strip_prefix("checkpoint_") {
            if let Some(stem) = stem.strip_suffix(".json") {
                if let Ok(id) = stem.parse::<u32>() {
                    ids.push(id);
                }
            }
        }
    }
    ids.sort();
    Ok(ids)
}
