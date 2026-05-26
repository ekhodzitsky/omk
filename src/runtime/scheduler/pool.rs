use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

use super::task::{Task, TaskId};

/// Default pool name when a task does not specify one.
pub const DEFAULT_POOL_NAME: &str = "default";

/// Configuration for a single agent pool.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentPool {
    pub name: String,
    #[serde(default = "default_max_workers")]
    pub max_workers: usize,
    #[serde(default)]
    pub max_disk_gb: Option<u64>,
    #[serde(default)]
    pub queue_capacity: Option<usize>,
    #[serde(default)]
    pub max_memory_mb: Option<u64>,
}

fn default_max_workers() -> usize {
    num_cpus::get()
}

impl Default for AgentPool {
    fn default() -> Self {
        Self {
            name: DEFAULT_POOL_NAME.to_string(),
            max_workers: default_max_workers(),
            max_disk_gb: None,
            queue_capacity: None,
            max_memory_mb: None,
        }
    }
}

impl AgentPool {
    /// Create an implicit default pool when none is configured.
    pub fn implicit_default() -> Self {
        Self::default()
    }
}

/// A task waiting for a pool slot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueuedTask {
    pub task_id: TaskId,
    pub priority: i32,
    pub enqueued_at: DateTime<Utc>,
}

/// Internal mutable state for a single pool.
#[derive(Debug, Clone)]
pub struct PoolState {
    pub config: AgentPool,
    pub active: HashSet<TaskId>,
    pub queued: VecDeque<QueuedTask>,
    pub disk_usage_bytes: u64,
}

impl PoolState {
    fn new(config: AgentPool) -> Self {
        // Guard against max_workers = 0 which would deadlock all tasks.
        let mut config = config;
        if config.max_workers == 0 {
            config.max_workers = 1;
        }
        Self {
            config,
            active: HashSet::new(),
            queued: VecDeque::new(),
            disk_usage_bytes: 0,
        }
    }

    fn can_admit(&self) -> bool {
        if self.active.len() >= self.config.max_workers {
            return false;
        }
        if let Some(max_disk) = self.config.max_disk_gb {
            let max_bytes = max_disk.saturating_mul(1_000_000_000);
            if self.disk_usage_bytes >= max_bytes {
                return false;
            }
        }
        true
    }

    fn enqueue(&mut self, task: QueuedTask) -> Result<(), PoolError> {
        // Deduplicate: silently succeed if already queued.
        if self.queued.iter().any(|q| q.task_id == task.task_id) {
            return Ok(());
        }
        if let Some(cap) = self.config.queue_capacity {
            if self.queued.len() >= cap {
                return Err(PoolError::QueueFull {
                    pool: self.config.name.clone(),
                    capacity: cap,
                });
            }
        }
        // Insert by priority: higher priority goes first, maintaining FIFO for equal priority.
        let pos = self
            .queued
            .iter()
            .position(|q| q.priority < task.priority)
            .unwrap_or(self.queued.len());
        self.queued.insert(pos, task);
        Ok(())
    }

    /// Atomically check admission and record if the slot is available.
    /// Returns true if the task was admitted, false otherwise.
    fn try_admit(&mut self, task_id: &TaskId) -> bool {
        if !self.can_admit() {
            return false;
        }
        self.active.insert(task_id.clone());
        true
    }

    fn release_slot(&mut self, task_id: &TaskId) -> Option<QueuedTask> {
        self.active.remove(task_id);
        // Promote the next queued task, if any.
        self.queued.pop_front()
    }

    fn update_disk_usage(&mut self, delta_bytes: i64) {
        if delta_bytes >= 0 {
            self.disk_usage_bytes = self.disk_usage_bytes.saturating_add(delta_bytes as u64);
        } else {
            self.disk_usage_bytes = self
                .disk_usage_bytes
                .saturating_sub(delta_bytes.unsigned_abs());
        }
    }
}

/// Typed errors for pool operations.
#[derive(Debug, thiserror::Error)]
pub enum PoolError {
    #[error("queue full for pool {pool}: capacity {capacity}")]
    QueueFull { pool: String, capacity: usize },
    #[error("unknown pool: {0}")]
    UnknownPool(String),
    #[error("task {task_id} already active in pool {pool}")]
    AlreadyActive { pool: String, task_id: TaskId },
}

/// Snapshot of a pool's current status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoolStatus {
    pub name: String,
    pub max_workers: usize,
    pub active_count: usize,
    pub queued_count: usize,
    pub disk_usage_bytes: u64,
    pub max_disk_gb: Option<u64>,
    pub queue_capacity: Option<usize>,
}

/// Observable pool events.
#[derive(Debug, Clone)]
pub enum PoolEvent {
    TaskAdmitted { pool: String, task_id: TaskId },
    TaskQueued { pool: String, task_id: TaskId },
    TaskReleased { pool: String, task_id: TaskId },
    QueueDrained { pool: String, task_id: TaskId },
}

/// Action sent to the pool manager when a task slot is released.
#[derive(Debug, Clone)]
pub enum PoolAction {
    Release {
        pool: String,
        task_id: TaskId,
        disk_delta: i64,
    },
}

/// Manages admission and resource tracking for all pools.
#[derive(Debug, Clone)]
pub struct PoolManager {
    inner: Arc<RwLock<HashMap<String, PoolState>>>,
}

impl PoolManager {
    /// Create a new manager from a map of pool configurations.
    /// If no "default" pool is provided, an implicit default is inserted.
    pub fn new(configs: HashMap<String, AgentPool>) -> Self {
        let mut pools = HashMap::new();
        for (name, config) in configs {
            pools.insert(name.clone(), PoolState::new(config));
        }
        if !pools.contains_key(DEFAULT_POOL_NAME) {
            pools.insert(
                DEFAULT_POOL_NAME.to_string(),
                PoolState::new(AgentPool::implicit_default()),
            );
        }
        Self {
            inner: Arc::new(RwLock::new(pools)),
        }
    }

    /// Resolve a pool name, falling back to default for unknown pools.
    fn resolve_pool_name<'a>(
        &self,
        guard: &'a HashMap<String, PoolState>,
        pool_name: &str,
    ) -> &'a PoolState {
        guard
            .get(pool_name)
            .or_else(|| guard.get(DEFAULT_POOL_NAME))
            .expect("default pool always exists")
    }

    fn resolve_pool_name_mut<'a>(
        &self,
        guard: &'a mut HashMap<String, PoolState>,
        pool_name: &str,
    ) -> &'a mut PoolState {
        if guard.contains_key(pool_name) {
            guard.get_mut(pool_name)
        } else {
            guard.get_mut(DEFAULT_POOL_NAME)
        }
        .expect("default pool always exists")
    }

    /// Non-blocking admission check. Returns `true` if the task may proceed.
    pub async fn can_admit(&self, pool_name: &str, _task_id: &TaskId) -> bool {
        let guard = self.inner.read().await;
        self.resolve_pool_name(&guard, pool_name).can_admit()
    }

    /// Enqueue a task when admission is denied.
    pub async fn enqueue(&self, pool_name: &str, task: &Task) -> Result<(), PoolError> {
        let mut guard = self.inner.write().await;
        let state = self.resolve_pool_name_mut(&mut guard, pool_name);
        let queued = QueuedTask {
            task_id: task.id.clone(),
            priority: task.priority,
            enqueued_at: Utc::now(),
        };
        let was_new = !state.queued.iter().any(|q| q.task_id == task.id);
        state.enqueue(queued)?;
        if was_new {
            info!(
                pool = %pool_name,
                task_id = %task.id,
                queued = state.queued.len(),
                "Task queued for pool slot"
            );
        }
        Ok(())
    }

    /// Atomically try to admit a task. Returns `true` if admitted.
    /// This is the only safe way to claim a pool slot; it prevents the
    /// TOCTOU race between `can_admit()` and `record_admission()`.
    pub async fn try_admit(&self, pool_name: &str, task_id: &TaskId) -> bool {
        let mut guard = self.inner.write().await;
        let state = self.resolve_pool_name_mut(&mut guard, pool_name);
        if state.try_admit(task_id) {
            info!(
                pool = %pool_name,
                task_id = %task_id,
                active = state.active.len(),
                max = state.config.max_workers,
                "Task admitted to pool"
            );
            true
        } else {
            false
        }
    }

    /// Release a slot when a task completes/fails/cancels.
    /// Returns the next queued task that should be promoted, if any.
    pub async fn release_slot(
        &self,
        pool_name: &str,
        task_id: &TaskId,
    ) -> Result<Option<QueuedTask>, PoolError> {
        let mut guard = self.inner.write().await;
        let state = self.resolve_pool_name_mut(&mut guard, pool_name);
        let promoted = state.release_slot(task_id);
        if let Some(ref next) = promoted {
            info!(
                pool = %pool_name,
                released_task = %task_id,
                promoted_task = %next.task_id,
                active = state.active.len(),
                queued = state.queued.len(),
                "Pool slot released, task promoted from queue"
            );
        } else {
            info!(
                pool = %pool_name,
                released_task = %task_id,
                active = state.active.len(),
                queued = state.queued.len(),
                "Pool slot released, no queued tasks"
            );
        }
        Ok(promoted)
    }

    /// Update disk usage by a delta (positive or negative bytes).
    pub async fn update_disk_usage(&self, pool_name: &str, delta_bytes: i64) {
        let mut guard = self.inner.write().await;
        let state = self.resolve_pool_name_mut(&mut guard, pool_name);
        state.update_disk_usage(delta_bytes);
    }

    /// Get a snapshot of all pool statuses.
    pub async fn all_statuses(&self) -> Vec<PoolStatus> {
        let guard = self.inner.read().await;
        guard
            .values()
            .map(|s| PoolStatus {
                name: s.config.name.clone(),
                max_workers: s.config.max_workers,
                active_count: s.active.len(),
                queued_count: s.queued.len(),
                disk_usage_bytes: s.disk_usage_bytes,
                max_disk_gb: s.config.max_disk_gb,
                queue_capacity: s.config.queue_capacity,
            })
            .collect()
    }

    /// Get status for a single pool. Falls back to default for unknown pools.
    pub async fn status(&self, pool_name: &str) -> Option<PoolStatus> {
        let guard = self.inner.read().await;
        let state = self.resolve_pool_name(&guard, pool_name);
        Some(PoolStatus {
            name: state.config.name.clone(),
            max_workers: state.config.max_workers,
            active_count: state.active.len(),
            queued_count: state.queued.len(),
            disk_usage_bytes: state.disk_usage_bytes,
            max_disk_gb: state.config.max_disk_gb,
            queue_capacity: state.config.queue_capacity,
        })
    }

    /// Apply an updated configuration. Already-active tasks are not affected.
    pub async fn apply_config(&self, configs: HashMap<String, AgentPool>) {
        let mut guard = self.inner.write().await;
        // Update existing pools and add new ones.
        for (name, config) in configs {
            if let Some(state) = guard.get_mut(&name) {
                state.config = config;
            } else {
                guard.insert(name.clone(), PoolState::new(config));
            }
        }
        // Ensure default pool always exists.
        if !guard.contains_key(DEFAULT_POOL_NAME) {
            guard.insert(
                DEFAULT_POOL_NAME.to_string(),
                PoolState::new(AgentPool::implicit_default()),
            );
        }
    }
}

impl Default for PoolManager {
    fn default() -> Self {
        Self::new(HashMap::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pool_manager_with(max_workers: usize, queue_capacity: Option<usize>) -> PoolManager {
        let mut configs = HashMap::new();
        configs.insert(
            "default".to_string(),
            AgentPool {
                name: "default".to_string(),
                max_workers,
                max_disk_gb: None,
                queue_capacity,
                max_memory_mb: None,
            },
        );
        PoolManager::new(configs)
    }

    fn fake_task(id: &str, priority: i32) -> Task {
        let mut task = Task::new(id, "test");
        task.priority = priority;
        task.pool = "default".to_string();
        task
    }

    #[tokio::test]
    async fn admission_max_workers() {
        let pm = pool_manager_with(2, None);
        let t1 = fake_task("t1", 0);
        let t2 = fake_task("t2", 0);
        let t3 = fake_task("t3", 0);

        assert!(pm.can_admit("default", &t1.id).await);
        assert!(pm.try_admit("default", &t1.id).await);

        assert!(pm.can_admit("default", &t2.id).await);
        assert!(pm.try_admit("default", &t2.id).await);

        assert!(!pm.can_admit("default", &t3.id).await);
        pm.enqueue("default", &t3).await.unwrap();

        let statuses = pm.all_statuses().await;
        assert_eq!(statuses[0].active_count, 2);
        assert_eq!(statuses[0].queued_count, 1);
    }

    #[tokio::test]
    async fn queue_ordering_fifo_with_priority() {
        let pm = pool_manager_with(1, None);
        let t1 = fake_task("t1", 0);
        let t2 = fake_task("t2", 5);
        let t3 = fake_task("t3", 0);

        assert!(pm.try_admit("default", &t1.id).await);
        assert!(!pm.can_admit("default", &t2.id).await);
        pm.enqueue("default", &t2).await.unwrap();
        pm.enqueue("default", &t3).await.unwrap();

        let promoted = pm.release_slot("default", &t1.id).await.unwrap();
        assert_eq!(promoted.unwrap().task_id, "t2"); // higher priority first
    }

    #[tokio::test]
    async fn slot_release_promotes_queued() {
        let pm = pool_manager_with(1, None);
        let t1 = fake_task("t1", 0);
        let t2 = fake_task("t2", 0);

        assert!(pm.try_admit("default", &t1.id).await);
        pm.enqueue("default", &t2).await.unwrap();

        let promoted = pm.release_slot("default", &t1.id).await.unwrap();
        assert_eq!(promoted.unwrap().task_id, "t2");
        assert_eq!(pm.status("default").await.unwrap().queued_count, 0);
    }

    #[tokio::test]
    async fn disk_tracking() {
        let pm = pool_manager_with(10, None);
        pm.update_disk_usage("default", 300_000_000).await;
        let s = pm.status("default").await.unwrap();
        assert_eq!(s.disk_usage_bytes, 300_000_000);

        pm.update_disk_usage("default", -100_000_000).await;
        let s = pm.status("default").await.unwrap();
        assert_eq!(s.disk_usage_bytes, 200_000_000);
    }

    #[tokio::test]
    async fn zero_config_implicit_default() {
        let pm = PoolManager::new(HashMap::new());
        let s = pm.status("default").await.unwrap();
        assert_eq!(s.name, "default");
        assert_eq!(s.max_workers, num_cpus::get());
    }

    #[tokio::test]
    async fn queue_capacity_rejects_overflow() {
        let pm = pool_manager_with(1, Some(1));
        let t1 = fake_task("t1", 0);
        let t2 = fake_task("t2", 0);
        let t3 = fake_task("t3", 0);

        assert!(pm.try_admit("default", &t1.id).await);
        pm.enqueue("default", &t2).await.unwrap();
        let err = pm.enqueue("default", &t3).await.unwrap_err();
        assert!(matches!(err, PoolError::QueueFull { .. }));
    }

    #[tokio::test]
    async fn race_simulation_ten_workers_three_slots() {
        let pm = pool_manager_with(3, None);
        let mut handles = Vec::new();

        for i in 0..10 {
            let pm = pm.clone();
            let task_id = format!("task-{}", i);
            handles.push(tokio::spawn(async move {
                if pm.can_admit("default", &task_id).await {
                    pm.try_admit("default", &task_id).await
                } else {
                    false
                }
            }));
        }

        let mut results = Vec::new();
        for h in handles {
            results.push(h.await.unwrap_or(false));
        }

        let admitted = results.iter().filter(|&&b| b).count();
        assert_eq!(admitted, 3, "exactly 3 tasks should be admitted");

        let status = pm.status("default").await.unwrap();
        assert_eq!(status.active_count, 3);
    }

    #[tokio::test]
    async fn unknown_pool_falls_back_to_default() {
        let pm = PoolManager::new(HashMap::new());
        assert!(pm.can_admit("nonexistent", &"t1".to_string()).await);
    }

    #[tokio::test]
    async fn enqueue_deduplicates() {
        let pm = pool_manager_with(1, None);
        let t1 = fake_task("t1", 0);

        assert!(pm.try_admit("default", &t1.id).await);
        let t2 = fake_task("t2", 0);
        pm.enqueue("default", &t2).await.unwrap();
        pm.enqueue("default", &t2).await.unwrap(); // duplicate
        pm.enqueue("default", &t2).await.unwrap(); // duplicate again

        let status = pm.status("default").await.unwrap();
        assert_eq!(status.queued_count, 1);
    }

    #[tokio::test]
    async fn release_slot_with_unknown_pool_falls_back() {
        let pm = PoolManager::new(HashMap::new());
        // t1 admitted to default pool via fallback
        assert!(pm.try_admit("nonexistent", &"t1".to_string()).await);
        let promoted = pm
            .release_slot("nonexistent", &"t1".to_string())
            .await
            .unwrap();
        assert!(promoted.is_none());
    }

    #[tokio::test]
    async fn zero_max_workers_normalized_to_one() {
        let mut configs = HashMap::new();
        configs.insert(
            "default".to_string(),
            AgentPool {
                name: "default".to_string(),
                max_workers: 0,
                max_disk_gb: None,
                queue_capacity: None,
                max_memory_mb: None,
            },
        );
        let pm = PoolManager::new(configs);
        let s = pm.status("default").await.unwrap();
        assert_eq!(s.max_workers, 1);
        assert!(pm.try_admit("default", &"t1".to_string()).await);
        assert!(!pm.try_admit("default", &"t2".to_string()).await);
    }

    #[tokio::test]
    async fn try_admit_is_atomic() {
        let pm = pool_manager_with(1, None);
        let mut handles = Vec::new();

        for i in 0..5 {
            let pm = pm.clone();
            let task_id = format!("task-{}", i);
            handles.push(tokio::spawn(async move {
                pm.try_admit("default", &task_id).await
            }));
        }

        let mut admitted = 0;
        for h in handles {
            if h.await.unwrap_or(false) {
                admitted += 1;
            }
        }

        assert_eq!(admitted, 1, "exactly 1 task should be atomically admitted");
        let status = pm.status("default").await.unwrap();
        assert_eq!(status.active_count, 1);
    }
}
