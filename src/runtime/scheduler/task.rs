use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

/// Default maximum retries before marking a task as Failed.
pub const DEFAULT_MAX_RETRIES: u32 = 2;

fn default_max_attempts() -> u32 {
    DEFAULT_MAX_RETRIES
}

fn default_backoff_seconds() -> u64 {
    1
}

fn default_backoff_multiplier() -> f64 {
    2.0
}

fn default_terminal() -> bool {
    true
}

/// Unique identifier for a task within a run.
pub type TaskId = String;

/// State of a task in the scheduler lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskState {
    /// Task is waiting for dependencies or a worker claim.
    Pending,
    /// A worker has claimed this task but not started yet.
    Claimed,
    /// Worker is actively executing this task.
    Running,
    /// Task completed successfully.
    Completed,
    /// Task failed (may be retried).
    Failed,
    /// Task was cancelled by user or system.
    Cancelled,
}

impl TaskState {
    /// Returns true if the task is in a terminal state.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            TaskState::Completed | TaskState::Failed | TaskState::Cancelled
        )
    }

    /// Returns true if the task can be claimed by a worker.
    pub fn claimable(&self) -> bool {
        matches!(self, TaskState::Pending)
    }
}

/// Retry policy with exponential backoff.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RetryPolicy {
    #[serde(default = "default_max_attempts")]
    pub max_attempts: u32,
    #[serde(default = "default_backoff_seconds")]
    pub backoff_seconds: u64,
    #[serde(default = "default_backoff_multiplier")]
    pub backoff_multiplier: f64,
    #[serde(default = "default_terminal")]
    pub terminal_on_permanent_failure: bool,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: DEFAULT_MAX_RETRIES,
            backoff_seconds: 1,
            backoff_multiplier: 2.0,
            terminal_on_permanent_failure: true,
        }
    }
}

impl RetryPolicy {
    /// Calculate the backoff duration for a given attempt number.
    pub fn next_backoff(&self, attempt: u32) -> Duration {
        if attempt == 0 {
            return Duration::from_secs(self.backoff_seconds);
        }
        let multiplier = self
            .backoff_multiplier
            .powi(attempt.saturating_sub(1) as i32);
        let secs = (self.backoff_seconds as f64 * multiplier) as u64;
        Duration::from_secs(secs)
    }
}

/// A single unit of work in the scheduler.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: TaskId,
    pub name: String,
    pub description: String,
    /// Task IDs that must complete before this task can start.
    pub dependencies: Vec<TaskId>,
    /// Higher number = higher priority (executed before lower priority).
    pub priority: i32,
    /// Which worker (if any) currently owns this task.
    pub owner: Option<String>,
    /// When the current lease expires. If past, the task can be reclaimed.
    pub lease_expires: Option<DateTime<Utc>>,
    /// How many times this task has been retried.
    pub retry_count: u32,
    /// Retry policy for this task.
    #[serde(default)]
    pub retry_policy: RetryPolicy,
    /// Maximum number of retries before marking as Failed.
    /// Kept for backward compatibility during deserialization.
    #[serde(default = "default_max_attempts", skip_serializing)]
    pub max_retries: u32,
    pub state: TaskState,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    /// File paths this task is expected to read (for conflict detection).
    pub read_set: Vec<String>,
    /// File paths this task is expected to write (for conflict detection).
    pub write_set: Vec<String>,
    /// Arbitrary metadata for the task (e.g., command, mode, role).
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

impl Task {
    pub fn new(id: impl Into<TaskId>, name: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: id.into(),
            name: name.into(),
            description: String::new(),
            dependencies: Vec::new(),
            priority: 0,
            owner: None,
            lease_expires: None,
            retry_count: 0,
            retry_policy: RetryPolicy::default(),
            max_retries: DEFAULT_MAX_RETRIES,
            state: TaskState::Pending,
            created_at: now,
            started_at: None,
            completed_at: None,
            read_set: Vec::new(),
            write_set: Vec::new(),
            extra: HashMap::new(),
        }
    }

    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    pub fn with_dependencies(mut self, deps: Vec<TaskId>) -> Self {
        self.dependencies = deps;
        self
    }

    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_max_retries(mut self, max: u32) -> Self {
        self.retry_policy.max_attempts = max;
        self.max_retries = max;
        self
    }

    pub fn with_retry_policy(mut self, policy: RetryPolicy) -> Self {
        self.retry_policy = policy;
        self
    }

    pub fn with_write_set(mut self, paths: Vec<String>) -> Self {
        self.write_set = paths;
        self
    }

    pub fn with_read_set(mut self, paths: Vec<String>) -> Self {
        self.read_set = paths;
        self
    }

    /// Resolve the effective retry policy, respecting backward compatibility.
    pub fn effective_retry_policy(&self) -> RetryPolicy {
        let mut policy = self.retry_policy.clone();
        if self.max_retries != DEFAULT_MAX_RETRIES && self.retry_policy == RetryPolicy::default() {
            policy.max_attempts = self.max_retries;
        }
        policy
    }

    /// Check if all dependencies are in a terminal (success or failure) state.
    pub fn dependencies_met(&self, tasks: &HashMap<TaskId, Task>) -> bool {
        self.dependencies.iter().all(|dep_id| {
            tasks
                .get(dep_id)
                .map(|t| t.state.is_terminal())
                .unwrap_or(true) // Missing dependency = treat as satisfied
        })
    }

    /// Check if all dependencies completed successfully.
    pub fn dependencies_succeeded(&self, tasks: &HashMap<TaskId, Task>) -> bool {
        self.dependencies.iter().all(|dep_id| {
            tasks
                .get(dep_id)
                .map(|t| t.state == TaskState::Completed)
                .unwrap_or(false)
        })
    }

    /// True if the lease has expired (or no lease exists).
    pub fn lease_expired(&self) -> bool {
        match self.lease_expires {
            None => true,
            Some(expiry) => Utc::now() > expiry,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_state_terminal() {
        assert!(TaskState::Completed.is_terminal());
        assert!(TaskState::Failed.is_terminal());
        assert!(TaskState::Cancelled.is_terminal());
        assert!(!TaskState::Pending.is_terminal());
        assert!(!TaskState::Claimed.is_terminal());
        assert!(!TaskState::Running.is_terminal());
    }

    #[test]
    fn task_dependencies_met() {
        let mut tasks = HashMap::new();
        let mut dep = Task::new("dep1", "dependency");
        dep.state = TaskState::Completed;
        tasks.insert("dep1".to_string(), dep);

        let task = Task::new("main", "main task").with_dependencies(vec!["dep1".to_string()]);
        assert!(task.dependencies_met(&tasks));
        assert!(task.dependencies_succeeded(&tasks));
    }

    #[test]
    fn task_dependencies_not_met() {
        let mut tasks = HashMap::new();
        let dep = Task::new("dep1", "dependency"); // Pending
        tasks.insert("dep1".to_string(), dep);

        let task = Task::new("main", "main task").with_dependencies(vec!["dep1".to_string()]);
        assert!(!task.dependencies_met(&tasks));
        assert!(!task.dependencies_succeeded(&tasks));
    }

    #[test]
    fn task_lease_expired() {
        let mut task = Task::new("t1", "test");
        assert!(task.lease_expired());

        task.lease_expires = Some(Utc::now() + chrono::Duration::seconds(30));
        assert!(!task.lease_expired());

        task.lease_expires = Some(Utc::now() - chrono::Duration::seconds(1));
        assert!(task.lease_expired());
    }

    #[test]
    fn retry_policy_next_backoff() {
        let policy = RetryPolicy {
            max_attempts: 3,
            backoff_seconds: 2,
            backoff_multiplier: 2.0,
            terminal_on_permanent_failure: true,
        };
        assert_eq!(policy.next_backoff(0).as_secs(), 2);
        assert_eq!(policy.next_backoff(1).as_secs(), 2);
        assert_eq!(policy.next_backoff(2).as_secs(), 4);
        assert_eq!(policy.next_backoff(3).as_secs(), 8);
    }

    #[test]
    fn task_backward_compat_max_retries() {
        let task = Task::new("t1", "test").with_max_retries(5);
        assert_eq!(task.effective_retry_policy().max_attempts, 5);
        assert_eq!(task.max_retries, 5);
    }

    #[test]
    fn task_effective_retry_policy_prefers_explicit_policy() {
        let task = Task::new("t1", "test").with_retry_policy(RetryPolicy {
            max_attempts: 7,
            ..Default::default()
        });
        assert_eq!(task.effective_retry_policy().max_attempts, 7);
    }

    #[test]
    fn task_legacy_deserialization_migration() {
        // Simulate old JSON that only had max_retries
        let json = r#"{
            "id": "t1",
            "name": "test",
            "description": "",
            "dependencies": [],
            "priority": 0,
            "owner": null,
            "lease_expires": null,
            "retry_count": 0,
            "max_retries": 5,
            "state": "pending",
            "created_at": "2024-01-01T00:00:00Z",
            "started_at": null,
            "completed_at": null,
            "read_set": [],
            "write_set": []
        }"#;
        let task: Task = serde_json::from_str(json).unwrap();
        assert_eq!(task.effective_retry_policy().max_attempts, 5);
    }
}
