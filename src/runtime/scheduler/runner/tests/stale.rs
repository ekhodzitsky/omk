use tempfile::TempDir;
use tokio::time::{timeout, Duration};

use crate::runtime::events::EventKind;
use crate::runtime::scheduler::task::{Task, TaskState};

use super::*;

#[tokio::test]
async fn test_recovered_stale_task_prefers_different_worker() {
    let tmp = TempDir::new().unwrap();
    let mut runner = make_runner(&tmp).await;
    runner.set_lease_seconds(-1);
    runner.seed_task("Recover this stale task");

    let worker_a = make_spec(&tmp, "worker-a").await;
    let worker_b = make_spec(&tmp, "worker-b").await;
    runner
        .dispatch_to_workers(&[worker_a.clone(), worker_b.clone()])
        .await
        .unwrap();

    // Manually recover stale leases so we can record stale owners for re-dispatch.
    let recovered = runner.claim_store.recover_stale_leases_with_owners();
    assert_eq!(recovered.len(), 1);
    for recovery in &recovered {
        if let Some(task) = runner.claim_store.get(&recovery.task_id) {
            runner.ownership.release_task(task);
        }
        if let Some(stale_owner) = recovery.stale_owner.as_deref() {
            runner
                .stale_task_owners
                .insert(recovery.task_id.clone(), stale_owner.to_string());
        }
    }

    runner
        .dispatch_to_workers(&[worker_a.clone(), worker_b.clone()])
        .await
        .unwrap();

    assert_task_state(&runner, "task-1", TaskState::Running, Some("worker-b"));

    let inbox_b = read_inbox(&worker_b).await;
    assert!(
        inbox_b.contains("task-1"),
        "recovered task should be sent to the non-stale worker when available"
    );
}

#[tokio::test]
async fn test_recovered_stale_worker_is_quarantined_for_future_dispatch() {
    let tmp = TempDir::new().unwrap();
    let mut runner = make_runner(&tmp).await;
    runner.set_lease_seconds(-1);
    runner.seed_task("Recover this stale task");

    let worker_a = make_spec(&tmp, "worker-a").await;
    let worker_b = make_spec(&tmp, "worker-b").await;
    runner
        .dispatch_to_workers(&[worker_a.clone(), worker_b.clone()])
        .await
        .unwrap();

    runner.recover_stale_leases().await.unwrap();
    runner.claim_store.insert(
        Task::new("task-2", "second task")
            .with_description("must not be sent to quarantined worker"),
    );

    runner
        .dispatch_to_workers(&[worker_a.clone(), worker_b.clone()])
        .await
        .unwrap();

    let worker_a_inbox = read_inbox(&worker_a).await;
    assert!(
        !worker_a_inbox.contains("task-2"),
        "quarantined stale worker must not receive future tasks"
    );
    assert!(
        stale_cleanup_marker(&tmp, "worker-a").exists(),
        "stale worker cleanup marker should be durable"
    );

    assert_scheduler_event_present(&tmp, EventKind::WorkerDead, "worker-a").await;
}

#[tokio::test]
async fn test_run_fails_instead_of_hanging_when_all_workers_go_stale() {
    let tmp = TempDir::new().unwrap();
    let mut runner = make_runner(&tmp).await;
    runner.set_lease_seconds(-1);
    runner.seed_task("No live worker can finish this");

    let worker = make_spec(&tmp, "worker-only").await;
    let summary = timeout(
        Duration::from_secs(5),
        runner.run(std::slice::from_ref(&worker)),
    )
    .await
    .expect("runner should not hang after all workers go stale")
    .unwrap();

    assert_eq!(summary.completed, 0);
    assert_eq!(summary.failed, 1);
    assert_eq!(summary.total, 1);
    assert!(stale_cleanup_marker(&tmp, "worker-only").exists());
}
