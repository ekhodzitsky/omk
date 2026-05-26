pub mod error;
pub mod handle;
pub mod migrations;
pub mod repo;
pub mod schema;
pub mod transaction;
pub mod types;

pub use error::DbError;
pub use handle::DbHandle;
pub use transaction::DbTransaction;
pub use types::{
    ArtifactRecord, BudgetCheckpoint, DecisionRecord, EventRecord, GoalFilter, GoalRecord,
    GoalSummary, ProofRecord, TaskRecord,
};

// Re-export repository traits so consumers don't need to reach into repo::.
pub use repo::{
    artifact::ArtifactRepo,
    budget::BudgetRepo,
    circuit_breaker::{CircuitBreakerRepo, CircuitBreakerRepoImpl},
    decision::DecisionRepo,
    event::EventRepo,
    goal::GoalRepo,
    proof::ProofRepo,
    slice_lease::{SliceLease, SliceLeaseRepo},
    task::TaskRepo,
};

use repo::{
    artifact::ArtifactRepoImpl, budget::BudgetRepoImpl, decision::DecisionRepoImpl,
    event::EventRepoImpl, goal::GoalRepoImpl, pool::PoolRepoImpl, proof::ProofRepoImpl,
    slice_lease::SliceLeaseRepoImpl, task::TaskRepoImpl,
};

use std::sync::OnceLock;

static GLOBAL_DB: OnceLock<DbHandle> = OnceLock::new();

/// Set the global database handle used by runtime/goal code that does not
/// receive an explicit `DbHandle` parameter. Called once during app startup.
pub fn set_global_db(db: DbHandle) -> Result<(), DbHandle> {
    GLOBAL_DB.set(db)
}

/// Access the global database handle, if one has been set.
pub fn global_db() -> Option<DbHandle> {
    GLOBAL_DB.get().cloned()
}

impl DbHandle {
    /// Access the goal repository directly (auto-commit mode).
    pub fn goal_repo(&self) -> GoalRepoImpl {
        GoalRepoImpl {
            conn: self.conn.clone(),
        }
    }

    /// Access the task repository directly (auto-commit mode).
    pub fn task_repo(&self) -> TaskRepoImpl {
        TaskRepoImpl {
            conn: self.conn.clone(),
        }
    }

    /// Access the event repository directly (auto-commit mode).
    pub fn event_repo(&self) -> EventRepoImpl {
        EventRepoImpl {
            conn: self.conn.clone(),
        }
    }

    /// Access the proof repository directly (auto-commit mode).
    pub fn proof_repo(&self) -> ProofRepoImpl {
        ProofRepoImpl {
            conn: self.conn.clone(),
        }
    }

    /// Access the budget repository directly (auto-commit mode).
    pub fn budget_repo(&self) -> BudgetRepoImpl {
        BudgetRepoImpl {
            conn: self.conn.clone(),
        }
    }

    /// Access the artifact repository directly (auto-commit mode).
    pub fn artifact_repo(&self) -> ArtifactRepoImpl {
        ArtifactRepoImpl {
            conn: self.conn.clone(),
        }
    }

    /// Access the decision repository directly (auto-commit mode).
    pub fn decision_repo(&self) -> DecisionRepoImpl {
        DecisionRepoImpl {
            conn: self.conn.clone(),
        }
    }

    /// Access the slice lease repository directly (auto-commit mode).
    pub fn slice_lease_repo(&self) -> SliceLeaseRepoImpl {
        SliceLeaseRepoImpl {
            conn: self.conn.clone(),
        }
    }

    /// Access the pool repository directly (auto-commit mode).
    pub fn pool_repo(&self) -> PoolRepoImpl {
        PoolRepoImpl {
            conn: self.conn.clone(),
        }
    }

    /// Access the circuit breaker repository directly (auto-commit mode).
    pub fn circuit_breaker_repo(&self) -> CircuitBreakerRepoImpl {
        CircuitBreakerRepoImpl {
            conn: self.conn.clone(),
        }
    }
}

#[cfg(test)]
mod tests;
