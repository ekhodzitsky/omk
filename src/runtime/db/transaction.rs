use super::error::DbError;
use super::repo::{
    artifact::ArtifactRepoImpl, budget::BudgetRepoImpl, event::EventRepoImpl, goal::GoalRepoImpl,
    proof::ProofRepoImpl, task::TaskRepoImpl,
};

/// An active database transaction.
///
/// Callers must explicitly invoke `commit` or `rollback`. If dropped without
/// either, a best-effort rollback is spawned on the current Tokio runtime and
/// a warning is logged.
#[must_use = "DbTransaction must be explicitly committed or rolled back"]
pub struct DbTransaction {
    pub(super) conn: tokio_rusqlite::Connection,
    pub(super) active: bool,
}

impl DbTransaction {
    /// Commit the transaction.
    pub async fn commit(mut self) -> Result<(), DbError> {
        if !self.active {
            return Err(DbError::TransactionExpired);
        }
        self.conn
            .call(|conn| -> Result<(), rusqlite::Error> {
                conn.execute("COMMIT", [])?;
                Ok(())
            })
            .await
            .map_err(DbError::Connection)?;
        self.active = false;
        Ok(())
    }

    /// Rollback the transaction.
    pub async fn rollback(mut self) -> Result<(), DbError> {
        if !self.active {
            return Err(DbError::TransactionExpired);
        }
        self.conn
            .call(|conn| -> Result<(), rusqlite::Error> {
                conn.execute("ROLLBACK", [])?;
                Ok(())
            })
            .await
            .map_err(DbError::Connection)?;
        self.active = false;
        Ok(())
    }

    /// Access the goal repository within this transaction.
    pub fn goal_repo(&self) -> GoalRepoImpl {
        GoalRepoImpl {
            conn: self.conn.clone(),
        }
    }

    /// Access the task repository within this transaction.
    pub fn task_repo(&self) -> TaskRepoImpl {
        TaskRepoImpl {
            conn: self.conn.clone(),
        }
    }

    /// Access the event repository within this transaction.
    pub fn event_repo(&self) -> EventRepoImpl {
        EventRepoImpl {
            conn: self.conn.clone(),
        }
    }

    /// Access the proof repository within this transaction.
    pub fn proof_repo(&self) -> ProofRepoImpl {
        ProofRepoImpl {
            conn: self.conn.clone(),
        }
    }

    /// Access the budget repository within this transaction.
    pub fn budget_repo(&self) -> BudgetRepoImpl {
        BudgetRepoImpl {
            conn: self.conn.clone(),
        }
    }

    /// Access the artifact repository within this transaction.
    pub fn artifact_repo(&self) -> ArtifactRepoImpl {
        ArtifactRepoImpl {
            conn: self.conn.clone(),
        }
    }
}

impl Drop for DbTransaction {
    fn drop(&mut self) {
        if self.active {
            tracing::warn!("DbTransaction dropped without explicit commit or rollback");
            if let Ok(handle) = tokio::runtime::Handle::try_current() {
                let conn = self.conn.clone();
                handle.spawn(async move {
                    let _ = conn
                        .call(|conn| -> Result<(), rusqlite::Error> {
                            let _ = conn.execute("ROLLBACK", []);
                            Ok(())
                        })
                        .await;
                });
            }
        }
    }
}

impl std::fmt::Debug for DbTransaction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DbTransaction")
            .field("active", &self.active)
            .finish_non_exhaustive()
    }
}
