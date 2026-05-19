pub mod error;
pub mod handle;
pub mod repo;
pub mod schema;
pub mod transaction;
pub mod types;

pub use error::DbError;
pub use handle::DbHandle;
pub use transaction::DbTransaction;
pub use types::{
    ArtifactRecord, BudgetCheckpoint, EventRecord, GoalFilter, GoalRecord, GoalSummary,
    ProofRecord, TaskRecord,
};

use repo::{
    artifact::ArtifactRepoImpl, budget::BudgetRepoImpl, event::EventRepoImpl, goal::GoalRepoImpl,
    proof::ProofRepoImpl, task::TaskRepoImpl,
};

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
}

#[cfg(test)]
mod tests;
