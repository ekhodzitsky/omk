use thiserror::Error;

/// Errors returned by the SQLite storage layer.
#[derive(Debug, Error)]
pub enum DbError {
    #[error("connection failed: {0}")]
    Connection(#[from] tokio_rusqlite::Error),

    #[error("rusqlite error: {0}")]
    Rusqlite(#[from] rusqlite::Error),

    #[error("goal not found: {0}")]
    GoalNotFound(String),

    #[error("task not found: {0}")]
    TaskNotFound(String),

    #[error("transaction already committed or rolled back")]
    TransactionExpired,

    #[error("migration failed: {0}")]
    Migration(String),

    #[error("invalid data: {0}")]
    InvalidData(String),
}
