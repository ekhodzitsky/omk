use anyhow::Result;
use chrono::Utc;
use std::path::{Path, PathBuf};
use uuid::Uuid;

use super::constants::GOAL_STATE_FILE;
use super::error::GoalStateError;
use super::types::GoalState;

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
        let mut state: Self =
            serde_json::from_str(&json).map_err(|e| GoalStateError::InvalidFormat {
                path: path.display().to_string(),
                reason: e.to_string(),
            })?;
        state.state_dir = goal_dir.to_path_buf();
        Ok(state)
    }
}

pub fn goals_dir() -> PathBuf {
    crate::runtime::config::omk_state_dir().join(super::constants::GOALS_DIR)
}

pub(crate) fn generate_goal_id() -> String {
    let suffix = Uuid::new_v4().to_string();
    format!(
        "goal-{}-{}",
        Utc::now().format("%Y%m%d-%H%M%S-%3f"),
        &suffix[..8]
    )
}

pub(crate) fn normalize_goal(goal: &str) -> String {
    goal.split_whitespace().collect::<Vec<_>>().join(" ")
}
