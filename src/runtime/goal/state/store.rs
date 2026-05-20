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
#[derive(Debug)]
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
        // Primary: SQLite (best-effort; never blocks on DB errors).
        match super::db_store::DbGoalStateStore::open().await {
            Ok(db) => {
                if let Err(e) = db.save(state).await {
                    tracing::warn!(
                        error = %e,
                        goal_id = %state.goal_id,
                        "DB goal save failed; keeping JSON backup only"
                    );
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "Failed to open goals DB; writing JSON backup only");
            }
        }

        // Always write JSON backup for backward compatibility.
        super::db_store::json_backup_save(state).await
    }

    async fn load(&self, goal_dir: &Path) -> Result<GoalState> {
        // Primary: JSON (canonical per-goal source of truth).
        // DB is fallback only when the JSON file is missing, so that external
        // tools which modify JSON directly continue to work and so that
        // corrupted/unreadable JSON surfaces as a real error rather than being
        // silently masked by a stale DB record.
        match super::db_store::json_backup_load(goal_dir).await {
            Ok(state) => return Ok(state),
            Err(e) => {
                let is_missing = e
                    .downcast_ref::<GoalStateError>()
                    .is_some_and(|ge| matches!(ge, GoalStateError::MissingFile { .. }));
                if !is_missing {
                    return Err(e);
                }
            }
        }

        match super::db_store::DbGoalStateStore::open().await {
            Ok(db) => match db.load(goal_dir).await {
                Ok(state) => return Ok(state),
                Err(e) => {
                    tracing::warn!(
                        goal_dir = %goal_dir.display(),
                        error = %e,
                        "DB load failed after JSON miss"
                    );
                }
            },
            Err(e) => {
                tracing::warn!(
                    goal_dir = %goal_dir.display(),
                    error = %e,
                    "Failed to open goals DB after JSON miss"
                );
            }
        }
        Err(GoalStateError::MissingFile {
            path: goal_dir
                .join(GOAL_STATE_FILE)
                .to_string_lossy()
                .to_string(),
        }
        .into())
    }

    async fn list(&self) -> Result<Vec<GoalState>> {
        // Primary: SQLite with JSON fallback.
        match super::db_store::DbGoalStateStore::open().await {
            Ok(db) => match db.list().await {
                Ok(goals) if !goals.is_empty() => return Ok(goals),
                Ok(_) => {} // empty DB, fall through to JSON scan
                Err(e) => tracing::warn!(error = %e, "DB list failed; falling back to JSON"),
            },
            Err(e) => tracing::warn!(error = %e, "Failed to open goals DB; falling back to JSON"),
        }

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
