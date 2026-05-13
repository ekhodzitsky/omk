use anyhow::Result;
use chrono::Utc;
use std::collections::HashMap;
use tracing::{info, warn};

use super::task::{Task, TaskId, TaskState};

/// Default lease duration for a claimed task.
pub const DEFAULT_LEASE_SECS: u64 = 300; // 5 minutes

/// In-memory claim store with file-backed persistence.
/// Tracks tasks, claims, and stale-lease recovery.
pub struct ClaimStore {
    tasks: HashMap<TaskId, Task>,
    lease_seconds: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveredLease {
    pub task_id: TaskId,
    pub stale_owner: Option<String>,
}

impl ClaimStore {
    pub fn new() -> Self {
        Self {
            tasks: HashMap::new(),
            lease_seconds: DEFAULT_LEASE_SECS as i64,
        }
    }

    pub fn with_lease_seconds(mut self, secs: i64) -> Self {
        self.lease_seconds = secs;
        self
    }

    pub fn set_lease_seconds(&mut self, secs: i64) {
        self.lease_seconds = secs;
    }

    pub fn insert(&mut self, task: Task) {
        self.tasks.insert(task.id.clone(), task);
    }

    pub fn get(&self, id: &TaskId) -> Option<&Task> {
        self.tasks.get(id)
    }

    pub fn get_mut(&mut self, id: &TaskId) -> Option<&mut Task> {
        self.tasks.get_mut(id)
    }

    pub fn tasks(&self) -> &HashMap<TaskId, Task> {
        &self.tasks
    }

    pub fn tasks_mut(&mut self) -> &mut HashMap<TaskId, Task> {
        &mut self.tasks
    }

    /// Find all tasks that are ready to be claimed:
    /// - State is Pending
    /// - All dependencies are in a terminal state
    /// - No active lease (or lease expired)
    ///
    /// Results are sorted by priority (highest first) and then by task id
    /// for deterministic dispatch order.
    pub fn ready_tasks(&self) -> Vec<&Task> {
        let mut ready: Vec<&Task> = self
            .tasks
            .values()
            .filter(|t| {
                t.state == TaskState::Pending
                    && t.dependencies_met(&self.tasks)
                    && t.lease_expired()
            })
            .collect();
        ready.sort_by(|a, b| b.priority.cmp(&a.priority).then_with(|| a.id.cmp(&b.id)));
        ready
    }

    /// Claim a task for a worker. Returns true if the claim succeeded.
    pub fn claim(&mut self, task_id: &TaskId, worker: &str) -> bool {
        // Pre-check with immutable borrow
        let can_claim = {
            let Some(task) = self.tasks.get(task_id) else {
                return false;
            };
            task.state == TaskState::Pending
                && task.lease_expired()
                && task.dependencies_met(&self.tasks)
        };

        if !can_claim {
            return false;
        }

        let Some(task) = self.tasks.get_mut(task_id) else {
            return false;
        };
        task.state = TaskState::Claimed;
        task.owner = Some(worker.to_string());
        task.lease_expires = Some(Utc::now() + chrono::Duration::seconds(self.lease_seconds));
        info!(task = %task_id, worker = %worker, "Task claimed");
        true
    }

    /// Mark a task as started by its owning worker.
    pub fn start(&mut self, task_id: &TaskId, worker: &str) -> bool {
        let Some(task) = self.tasks.get_mut(task_id) else {
            return false;
        };

        if task.owner.as_deref() != Some(worker) {
            warn!(task = %task_id, expected = ?task.owner, actual = %worker, "Worker mismatch on start");
            return false;
        }

        if task.state != TaskState::Claimed && task.state != TaskState::Pending {
            return false;
        }

        task.state = TaskState::Running;
        task.started_at = Some(Utc::now());
        info!(task = %task_id, worker = %worker, "Task started");
        true
    }

    /// Mark a task as completed.
    pub fn complete(&mut self, task_id: &TaskId, worker: &str) -> bool {
        let Some(task) = self.tasks.get_mut(task_id) else {
            return false;
        };

        if task.owner.as_deref() != Some(worker) {
            return false;
        }

        if task.state != TaskState::Running && task.state != TaskState::Claimed {
            return false;
        }

        task.state = TaskState::Completed;
        task.completed_at = Some(Utc::now());
        task.lease_expires = None;
        info!(task = %task_id, worker = %worker, "Task completed");
        true
    }

    /// Mark a task as failed. If retries remain, it returns to Pending with a backoff lease.
    pub fn fail(&mut self, task_id: &TaskId, worker: &str) -> bool {
        let Some(task) = self.tasks.get_mut(task_id) else {
            return false;
        };

        if task.owner.as_deref() != Some(worker) {
            return false;
        }

        if task.state != TaskState::Running && task.state != TaskState::Claimed {
            return false;
        }

        task.retry_count += 1;
        let policy = task.effective_retry_policy();
        if task.retry_count > policy.max_attempts {
            task.state = TaskState::Failed;
            task.completed_at = Some(Utc::now());
            task.lease_expires = None;
            warn!(task = %task_id, retries = task.retry_count, "Task failed permanently");
        } else {
            task.state = TaskState::Pending;
            task.owner = None;
            let backoff = policy.next_backoff(task.retry_count);
            let backoff_secs = backoff.as_secs().min(i64::MAX as u64) as i64;
            task.lease_expires = Some(Utc::now() + chrono::Duration::seconds(backoff_secs));
            task.started_at = None;
            info!(
                task = %task_id,
                retry = task.retry_count,
                backoff_secs = backoff.as_secs(),
                "Task failed, retry scheduled with backoff"
            );
        }
        true
    }

    /// Cancel a task and any dependent tasks that haven't started.
    pub fn cancel(&mut self, task_id: &TaskId) -> Result<bool> {
        let Some(task) = self.tasks.get_mut(task_id) else {
            return Ok(false);
        };

        if task.state.is_terminal() {
            return Ok(false);
        }

        task.state = TaskState::Cancelled;
        task.completed_at = Some(Utc::now());
        task.lease_expires = None;
        info!(task = %task_id, "Task cancelled");

        // Cascade cancellation to dependents that are still pending
        let dependents: Vec<TaskId> = self
            .tasks
            .values()
            .filter(|t| {
                t.dependencies.contains(task_id)
                    && matches!(t.state, TaskState::Pending | TaskState::Claimed)
            })
            .map(|t| t.id.clone())
            .collect();

        for dep_id in dependents {
            let _ = self.cancel(&dep_id);
        }

        Ok(true)
    }

    /// Recover any tasks with expired leases back to Pending.
    pub fn recover_stale_leases(&mut self) -> Vec<TaskId> {
        self.recover_stale_leases_with_owners()
            .into_iter()
            .map(|recovered| recovered.task_id)
            .collect()
    }

    /// Recover expired leases and preserve the worker that held the stale claim.
    pub fn recover_stale_leases_with_owners(&mut self) -> Vec<RecoveredLease> {
        let mut recovered = Vec::new();
        let now = Utc::now();

        for (id, task) in self.tasks.iter_mut() {
            if matches!(task.state, TaskState::Claimed | TaskState::Running) {
                if let Some(expiry) = task.lease_expires {
                    if now > expiry {
                        let stale_owner = task.owner.clone();
                        warn!(task = %id, owner = ?task.owner, "Stale lease recovered");
                        task.state = TaskState::Pending;
                        task.owner = None;
                        task.lease_expires = None;
                        task.started_at = None;
                        recovered.push(RecoveredLease {
                            task_id: id.clone(),
                            stale_owner,
                        });
                    }
                }
            }
        }

        recovered
    }

    /// Returns true if all tasks are in a terminal state.
    pub fn all_done(&self) -> bool {
        self.tasks.values().all(|t| t.state.is_terminal())
    }

    /// Summary of task states.
    pub fn summary(&self) -> TaskSummary {
        let mut summary = TaskSummary::default();
        for task in self.tasks.values() {
            match task.state {
                TaskState::Pending => summary.pending += 1,
                TaskState::Claimed => summary.claimed += 1,
                TaskState::Running => summary.running += 1,
                TaskState::Completed => summary.completed += 1,
                TaskState::Failed => summary.failed += 1,
                TaskState::Cancelled => summary.cancelled += 1,
            }
        }
        summary
    }
}

impl Default for ClaimStore {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Default)]
pub struct TaskSummary {
    pub pending: usize,
    pub claimed: usize,
    pub running: usize,
    pub completed: usize,
    pub failed: usize,
    pub cancelled: usize,
}

impl TaskSummary {
    pub fn total(&self) -> usize {
        self.pending + self.claimed + self.running + self.completed + self.failed + self.cancelled
    }

    pub fn done(&self) -> usize {
        self.completed + self.failed + self.cancelled
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::scheduler::task::RetryPolicy;

    #[test]
    fn claim_and_complete_flow() {
        let mut store = ClaimStore::new();
        let task = Task::new("t1", "test task");
        store.insert(task);

        assert!(store.claim(&"t1".to_string(), "worker-a"));
        assert_eq!(
            store.get(&"t1".to_string()).unwrap().state,
            TaskState::Claimed
        );

        assert!(store.start(&"t1".to_string(), "worker-a"));
        assert_eq!(
            store.get(&"t1".to_string()).unwrap().state,
            TaskState::Running
        );

        assert!(store.complete(&"t1".to_string(), "worker-a"));
        assert_eq!(
            store.get(&"t1".to_string()).unwrap().state,
            TaskState::Completed
        );
    }

    #[test]
    fn claim_wrong_worker_fails() {
        let mut store = ClaimStore::new();
        let task = Task::new("t1", "test task");
        store.insert(task);

        assert!(store.claim(&"t1".to_string(), "worker-a"));
        assert!(!store.start(&"t1".to_string(), "worker-b"));
    }

    #[test]
    fn fail_with_retry() {
        let mut store = ClaimStore::new();
        let task = Task::new("t1", "test task").with_retry_policy(RetryPolicy {
            max_attempts: 1,
            backoff_seconds: 0,
            ..Default::default()
        });
        store.insert(task);

        assert!(store.claim(&"t1".to_string(), "worker-a"));
        assert!(store.start(&"t1".to_string(), "worker-a"));

        // First failure -> retry
        assert!(store.fail(&"t1".to_string(), "worker-a"));
        assert_eq!(
            store.get(&"t1".to_string()).unwrap().state,
            TaskState::Pending
        );
        assert_eq!(store.get(&"t1".to_string()).unwrap().retry_count, 1);

        // Second failure -> permanent
        assert!(store.claim(&"t1".to_string(), "worker-a"));
        assert!(store.start(&"t1".to_string(), "worker-a"));
        assert!(store.fail(&"t1".to_string(), "worker-a"));
        assert_eq!(
            store.get(&"t1".to_string()).unwrap().state,
            TaskState::Failed
        );
    }

    #[test]
    fn stale_lease_recovery() {
        let mut store = ClaimStore::new().with_lease_seconds(-1); // Already expired
        let task = Task::new("t1", "test task");
        store.insert(task);

        assert!(store.claim(&"t1".to_string(), "worker-a"));
        assert_eq!(
            store.get(&"t1".to_string()).unwrap().state,
            TaskState::Claimed
        );

        let recovered = store.recover_stale_leases();
        assert_eq!(recovered, vec!["t1".to_string()]);
        assert_eq!(
            store.get(&"t1".to_string()).unwrap().state,
            TaskState::Pending
        );
    }

    #[test]
    fn cancel_cascades() {
        let mut store = ClaimStore::new();
        let dep = Task::new("dep", "dependency");
        let task = Task::new("main", "main").with_dependencies(vec!["dep".to_string()]);
        store.insert(dep);
        store.insert(task);

        assert!(store.cancel(&"dep".to_string()).unwrap());
        assert_eq!(
            store.get(&"dep".to_string()).unwrap().state,
            TaskState::Cancelled
        );
        assert_eq!(
            store.get(&"main".to_string()).unwrap().state,
            TaskState::Cancelled
        );
    }
}
