use chrono::{DateTime, Utc};

use super::pool::QueuedTask;
use super::task::TaskId;

/// Persistence interface for pool queue state.
///
/// Implementations allow queued tasks to survive graceful shutdown
/// and be restored on the next scheduler startup.
#[allow(async_fn_in_trait)]
pub trait PoolRepo {
    /// Persist the current queue for a given pool and run.
    async fn save_queue(
        &self,
        pool_name: &str,
        run_id: &str,
        queue: &[QueuedTask],
    ) -> Result<(), PoolRepoError>;

    /// Load previously saved queue entries for a run.
    async fn load_queue(&self, run_id: &str) -> Result<Vec<PoolQueueRecord>, PoolRepoError>;

    /// Delete all saved queue entries for a run.
    async fn delete_queue(&self, run_id: &str) -> Result<(), PoolRepoError>;

    /// Delete all saved queue entries for a pool within a run.
    async fn delete_pool_queue(&self, run_id: &str, pool_name: &str) -> Result<(), PoolRepoError>;
}

/// A single persisted queue entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoolQueueRecord {
    pub pool_name: String,
    pub task_id: TaskId,
    pub priority: i32,
    pub enqueued_at: DateTime<Utc>,
    pub run_id: String,
}

/// Errors from pool persistence operations.
#[derive(Debug, thiserror::Error)]
pub enum PoolRepoError {
    #[error("database error: {0}")]
    Database(String),
    #[error("serialization error: {0}")]
    Serialization(String),
}

impl From<crate::runtime::db::error::DbError> for PoolRepoError {
    fn from(e: crate::runtime::db::error::DbError) -> Self {
        PoolRepoError::Database(e.to_string())
    }
}

/// In-memory stub for tests and environments without SQLite.
#[derive(Debug, Default, Clone)]
pub struct PoolRepoStub {
    // Not needed for stub; everything is no-op.
}

#[allow(unused_variables)]
impl PoolRepo for PoolRepoStub {
    async fn save_queue(
        &self,
        _pool_name: &str,
        _run_id: &str,
        _queue: &[QueuedTask],
    ) -> Result<(), PoolRepoError> {
        Ok(())
    }

    async fn load_queue(&self, _run_id: &str) -> Result<Vec<PoolQueueRecord>, PoolRepoError> {
        Ok(Vec::new())
    }

    async fn delete_queue(&self, _run_id: &str) -> Result<(), PoolRepoError> {
        Ok(())
    }

    async fn delete_pool_queue(
        &self,
        _run_id: &str,
        _pool_name: &str,
    ) -> Result<(), PoolRepoError> {
        Ok(())
    }
}
