use std::sync::Arc;

use omk::runtime::scheduler::claim::ClaimStore;
use omk::runtime::scheduler::ownership::OwnershipMap;
use omk::runtime::scheduler::task::{Task, TaskState};
use tokio::sync::Mutex;
use tokio::time::{timeout, Duration};

/// In-memory scheduler simulator for deterministic race-condition testing.
/// Wraps `ClaimStore` and `OwnershipMap` in an async mutex so multiple
/// spawned tasks can contend for claims exactly as the real scheduler
/// serializes them.
struct SimScheduler {
    claims: ClaimStore,
    ownership: OwnershipMap,
}

impl SimScheduler {
    fn new() -> Self {
        Self {
            claims: ClaimStore::new(),
            ownership: OwnershipMap::new(),
        }
    }

    fn with_lease_seconds(mut self, secs: i64) -> Self {
        self.claims.set_lease_seconds(secs);
        self
    }

    fn insert(&mut self, task: Task) {
        self.claims.insert(task);
    }

    fn get_task(&self, task_id: &str) -> Option<&Task> {
        self.claims.get(&task_id.to_string())
    }

    fn raw_claim(&mut self, task_id: &str, worker: &str) -> bool {
        self.claims.claim(&task_id.to_string(), worker)
    }

    /// Atomically check ownership + claim store and claim if possible.
    fn try_claim(&mut self, task_id: &str, worker: &str) -> bool {
        let task_id_owned = task_id.to_string();
        let task = match self.claims.get(&task_id_owned) {
            Some(t) => t.clone(),
            None => return false,
        };
        if !self.ownership.would_conflict(&task, worker).is_empty() {
            return false;
        }
        if !self.claims.claim(&task_id_owned, worker) {
            return false;
        }
        if let Some(task) = self.claims.get(&task_id_owned) {
            self.ownership.register_task(task);
        }
        true
    }

    /// Complete a task and release its ownership.
    fn complete(&mut self, task_id: &str, worker: &str) -> bool {
        let task_id_owned = task_id.to_string();
        let ok = self.claims.complete(&task_id_owned, worker);
        if ok {
            if let Some(task) = self.claims.get(&task_id_owned) {
                self.ownership.release_task(task);
            }
        }
        ok
    }

    /// Recover stale leases and release associated ownership.
    fn recover_stale(&mut self) -> Vec<String> {
        let recovered = self.claims.recover_stale_leases_with_owners();
        for rec in &recovered {
            let task_id = rec.task_id.to_string();
            if let Some(task) = self.claims.get(&task_id) {
                self.ownership.release_task(task);
            }
        }
        recovered.into_iter().map(|r| r.task_id).collect()
    }
}

#[tokio::test]
async fn concurrent_claims_on_same_task_only_one_succeeds() {
    let mut sim = SimScheduler::new();
    sim.insert(Task::new("t1", "race task"));
    let sim = Arc::new(Mutex::new(sim));

    let mut handles = Vec::new();
    for i in 0..10 {
        let sim = Arc::clone(&sim);
        handles.push(tokio::spawn(async move {
            let mut guard = sim.lock().await;
            guard.try_claim("t1", &format!("worker-{i}"))
        }));
    }

    let mut results = Vec::new();
    for h in handles {
        results.push(h.await.unwrap());
    }

    let successes = results.iter().filter(|&&r| r).count();
    assert_eq!(successes, 1, "exactly one concurrent claim should succeed");

    let guard = sim.lock().await;
    let task = guard.get_task("t1").unwrap();
    assert_eq!(task.state, TaskState::Claimed);
    assert!(task.owner.as_deref().unwrap().starts_with("worker-"));
}

#[tokio::test]
async fn concurrent_claims_on_overlapping_writes_are_blocked_then_succeed_after_release() {
    let mut sim = SimScheduler::new();
    let t1 = Task::new("t1", "first writer").with_write_set(vec!["src/main.rs".to_string()]);
    let t2 = Task::new("t2", "second writer").with_write_set(vec!["src/main.rs".to_string()]);
    sim.insert(t1.clone());
    sim.insert(t2.clone());
    let sim = Arc::new(Mutex::new(sim));

    // Pre-claim t1 for worker-a.
    {
        let mut guard = sim.lock().await;
        assert!(guard.try_claim("t1", "worker-a"));
    }

    // Concurrently try to claim t2 for different workers — all should fail.
    let mut handles = Vec::new();
    for i in 0..5 {
        let sim = Arc::clone(&sim);
        handles.push(tokio::spawn(async move {
            let mut guard = sim.lock().await;
            guard.try_claim("t2", &format!("worker-{i}"))
        }));
    }

    let mut results = Vec::new();
    for h in handles {
        results.push(h.await.unwrap());
    }
    assert!(
        results.iter().all(|&r| !r),
        "all overlapping claims should fail while t1 holds the path"
    );

    // Release t1.
    {
        let mut guard = sim.lock().await;
        assert!(guard.complete("t1", "worker-a"));
    }

    // Now exactly one concurrent claim for t2 should win.
    let mut handles = Vec::new();
    for i in 0..5 {
        let sim = Arc::clone(&sim);
        handles.push(tokio::spawn(async move {
            let mut guard = sim.lock().await;
            guard.try_claim("t2", &format!("worker-{i}"))
        }));
    }

    let mut results = Vec::new();
    for h in handles {
        results.push(h.await.unwrap());
    }
    let successes = results.iter().filter(|&&r| r).count();
    assert_eq!(
        successes, 1,
        "exactly one claim should succeed after release"
    );
}

#[tokio::test]
async fn lease_expiry_makes_task_reclaimable() {
    let mut sim = SimScheduler::new().with_lease_seconds(1);
    let t1 = Task::new("t1", "leased task");
    sim.insert(t1.clone());
    let sim = Arc::new(Mutex::new(sim));

    {
        let mut guard = sim.lock().await;
        assert!(guard.try_claim("t1", "worker-a"));
        let task = guard.get_task("t1").unwrap();
        assert!(
            !task.lease_expired(),
            "freshly claimed lease should not be expired"
        );
    }

    // Poll until the lease expires or timeout.
    timeout(Duration::from_secs(5), async {
        loop {
            tokio::time::sleep(Duration::from_millis(20)).await;
            let guard = sim.lock().await;
            if let Some(task) = guard.get_task("t1") {
                if task.lease_expired() {
                    break;
                }
            }
        }
    })
    .await
    .expect("lease should expire within 5 seconds");

    {
        let mut guard = sim.lock().await;
        let recovered = guard.recover_stale();
        assert!(recovered.contains(&"t1".to_string()));

        let ready = guard.claims.ready_tasks();
        assert!(
            ready.iter().any(|t| t.id == "t1"),
            "expired task should appear in ready_tasks after recovery"
        );

        assert!(guard.raw_claim("t1", "worker-b"));
        let task = guard.get_task("t1").unwrap();
        assert_eq!(task.owner.as_deref(), Some("worker-b"));
    }
}

#[tokio::test]
async fn stale_worker_cleanup_invalidates_old_claims() {
    let mut sim = SimScheduler::new().with_lease_seconds(-1);
    let t1 = Task::new("t1", "stale task").with_write_set(vec!["src/main.rs".to_string()]);
    sim.insert(t1.clone());
    let sim = Arc::new(Mutex::new(sim));

    {
        let mut guard = sim.lock().await;
        assert!(guard.try_claim("t1", "worker-a"));
        let task = guard.get_task("t1").unwrap();
        assert_eq!(task.state, TaskState::Claimed);
        assert_eq!(task.owner.as_deref(), Some("worker-a"));
    }

    {
        let mut guard = sim.lock().await;
        let recovered = guard.recover_stale();
        assert_eq!(recovered, vec!["t1".to_string()]);

        let task = guard.get_task("t1").unwrap();
        assert_eq!(task.state, TaskState::Pending);
        assert_eq!(task.owner, None);
        assert!(task.lease_expired());

        // Ownership should be released so a new worker can claim a conflicting task.
        let t2 = Task::new("t2", "next task").with_write_set(vec!["src/main.rs".to_string()]);
        guard.insert(t2.clone());
        assert!(
            guard.try_claim("t2", "worker-b"),
            "stale cleanup should release ownership"
        );
    }
}

#[tokio::test]
async fn race_between_release_and_new_claim_maintains_invariants() {
    let mut sim = SimScheduler::new();
    let t1 = Task::new("t1", "releaser").with_write_set(vec!["src/main.rs".to_string()]);
    let t2 = Task::new("t2", "claimer").with_write_set(vec!["src/main.rs".to_string()]);
    sim.insert(t1.clone());
    sim.insert(t2.clone());
    let sim = Arc::new(Mutex::new(sim));

    {
        let mut guard = sim.lock().await;
        assert!(guard.try_claim("t1", "worker-a"));
    }

    // Spawn two tasks that race: one releases t1, the other tries to claim t2.
    let sim_a = Arc::clone(&sim);
    let handle_a = tokio::spawn(async move {
        let mut guard = sim_a.lock().await;
        guard.complete("t1", "worker-a")
    });

    let sim_b = Arc::clone(&sim);
    let handle_b = tokio::spawn(async move {
        let mut guard = sim_b.lock().await;
        guard.try_claim("t2", "worker-b")
    });

    let result_a = handle_a.await.unwrap();
    let result_b = handle_b.await.unwrap();

    assert!(result_a, "complete should always succeed");

    let guard = sim.lock().await;
    let task1 = guard.get_task("t1").unwrap();
    assert_eq!(task1.state, TaskState::Completed);

    let task2 = guard.get_task("t2").unwrap();
    if result_b {
        // A completed before B acquired the lock.
        assert_eq!(task2.state, TaskState::Claimed);
        assert_eq!(task2.owner.as_deref(), Some("worker-b"));
    } else {
        // B acquired the lock before A completed — ownership conflict blocked it.
        assert_eq!(task2.state, TaskState::Pending);
        assert_eq!(task2.owner, None);
    }
}
