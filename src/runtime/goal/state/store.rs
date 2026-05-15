use anyhow::Result;
use std::path::Path;

#[cfg(test)]
use std::collections::HashMap;
#[cfg(test)]
use std::path::PathBuf;

use super::constants::GOAL_STATE_FILE;
use super::error::GoalStateError;
use super::types::GoalState;

/// Storage backend contract for [`GoalState`].
///
/// Implementations isolate I/O so that unit tests can exercise logic
/// without touching the filesystem.
pub trait GoalStateStore: Send + Sync {
    /// Persist `state` to the store.
    fn save(&self, state: &GoalState) -> impl std::future::Future<Output = Result<()>> + Send;

    /// Load a [`GoalState`] from `goal_dir`.
    fn load(&self, goal_dir: &Path) -> impl std::future::Future<Output = Result<GoalState>> + Send;

    /// List all goals, newest first.
    fn list(&self) -> impl std::future::Future<Output = Result<Vec<GoalState>>> + Send;
}

/// Production implementation backed by the local filesystem.
pub struct FileSystemGoalStateStore;

impl FileSystemGoalStateStore {
    pub fn new() -> Self {
        Self
    }
}

impl Default for FileSystemGoalStateStore {
    fn default() -> Self {
        Self::new()
    }
}

impl GoalStateStore for FileSystemGoalStateStore {
    async fn save(&self, state: &GoalState) -> Result<()> {
        let path = state.state_dir.join(GOAL_STATE_FILE);
        let json = serde_json::to_string_pretty(state)?;
        crate::runtime::atomic::atomic_write(&path, json.as_bytes()).await
    }

    async fn load(&self, goal_dir: &Path) -> Result<GoalState> {
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
        let mut state: GoalState =
            serde_json::from_str(&json).map_err(|e| GoalStateError::InvalidFormat {
                path: path.display().to_string(),
                reason: e.to_string(),
            })?;
        state.state_dir = goal_dir.to_path_buf();
        Ok(state)
    }

    async fn list(&self) -> Result<Vec<GoalState>> {
        let dir = super::persistence::goals_dir();
        if !dir.exists() {
            return Ok(Vec::new());
        }

        let mut entries = tokio::fs::read_dir(&dir).await?;
        let mut goals = Vec::new();
        while let Some(entry) = entries.next_entry().await? {
            if entry.file_type().await?.is_dir() {
                match self.load(&entry.path()).await {
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
}

/// In-memory implementation for unit tests.
#[cfg(test)]
pub struct InMemoryGoalStateStore {
    inner: tokio::sync::Mutex<HashMap<PathBuf, GoalState>>,
}

#[cfg(test)]
impl InMemoryGoalStateStore {
    pub fn new() -> Self {
        Self {
            inner: tokio::sync::Mutex::new(HashMap::new()),
        }
    }

    #[allow(dead_code)]
    pub async fn insert(&self, goal_dir: PathBuf, state: GoalState) {
        self.inner.lock().await.insert(goal_dir, state);
    }
}

#[cfg(test)]
impl GoalStateStore for InMemoryGoalStateStore {
    async fn save(&self, state: &GoalState) -> Result<()> {
        self.inner
            .lock()
            .await
            .insert(state.state_dir.clone(), state.clone());
        Ok(())
    }

    async fn load(&self, goal_dir: &Path) -> Result<GoalState> {
        let inner = self.inner.lock().await;
        let mut state =
            inner
                .get(goal_dir)
                .cloned()
                .ok_or_else(|| GoalStateError::MissingFile {
                    path: goal_dir.display().to_string(),
                })?;
        state.state_dir = goal_dir.to_path_buf();
        Ok(state)
    }

    async fn list(&self) -> Result<Vec<GoalState>> {
        let inner = self.inner.lock().await;
        let mut goals: Vec<GoalState> = inner.values().cloned().collect();
        goals.sort_by(|a, b| {
            b.created_at
                .cmp(&a.created_at)
                .then_with(|| b.goal_id.cmp(&a.goal_id))
        });
        Ok(goals)
    }
}
