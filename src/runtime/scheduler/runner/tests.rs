use tempfile::TempDir;
use tokio::time::{timeout, Duration};

use crate::runtime::config::{EVENTS_FILE, WORKERS_DIR};
use crate::runtime::events::{EventKind, EventReader, EventWriter};
use crate::runtime::scheduler::runner::TeamRunner;
use crate::runtime::scheduler::task::{Task, TaskState};
use crate::runtime::worker::{ResultStatus, WorkerResult, WorkerSpec};
use tokio_util::sync::CancellationToken;

async fn make_runner(tmp: &TempDir) -> TeamRunner {
    let state_dir = tmp.path().join("state");
    let event_log = state_dir.join(EVENTS_FILE);
    tokio::fs::create_dir_all(&state_dir).await.unwrap();
    let event_writer = EventWriter::new(&event_log);
    TeamRunner::init(
        "run-test",
        "test task",
        tmp.path(),
        &state_dir,
        event_writer,
    )
    .await
    .unwrap()
}

async fn make_spec(tmp: &TempDir, name: &str) -> WorkerSpec {
    let dir = tmp.path().join("state").join(WORKERS_DIR).join(name);
    tokio::fs::create_dir_all(&dir).await.unwrap();
    WorkerSpec {
        name: name.to_string(),
        role: "coder".to_string(),
        inbox: dir.join("inbox.jsonl"),
        outbox: dir.join("outbox.jsonl"),
        heartbeat: dir.join("heartbeat.json"),
        project_dir: None,
    }
}

#[tokio::test]
async fn test_seed_task_and_dispatch_writes_inbox() {
    let tmp = TempDir::new().unwrap();
    let mut runner = make_runner(&tmp).await;
    runner.seed_task("Implement feature X");

    let spec = make_spec(&tmp, "worker-0").await;
    runner
        .dispatch_to_workers(std::slice::from_ref(&spec))
        .await
        .unwrap();

    let inbox = tokio::fs::read_to_string(&spec.inbox).await.unwrap();
    assert!(inbox.contains("task-1"));
    assert!(inbox.contains("Implement feature X"));

    let task = runner.claim_store.get(&"task-1".to_string()).unwrap();
    assert_eq!(task.state, TaskState::Running);
    assert_eq!(task.owner, Some("worker-0".to_string()));
}

#[tokio::test]
async fn test_dispatch_includes_structured_task_budget_in_inbox() {
    let tmp = TempDir::new().unwrap();
    let state_dir = tmp.path().join("state");
    let event_log = state_dir.join(EVENTS_FILE);
    tokio::fs::create_dir_all(&state_dir).await.unwrap();

    let mut task =
        Task::new("task-budgeted", "budgeted task").with_description("honor this task budget");
    task.extra
        .insert("budget_secs".to_string(), serde_json::json!(7));

    let mut runner = TeamRunner::init_with_tasks(
        "run-test",
        tmp.path(),
        &state_dir,
        EventWriter::new(&event_log),
        vec![task],
    )
    .await
    .unwrap();

    let spec = make_spec(&tmp, "worker-budget").await;
    runner
        .dispatch_to_workers(std::slice::from_ref(&spec))
        .await
        .unwrap();

    let inbox = tokio::fs::read_to_string(&spec.inbox).await.unwrap();
    let task_json: serde_json::Value = serde_json::from_str(inbox.lines().next().unwrap()).unwrap();
    assert_eq!(task_json["id"], "task-budgeted");
    assert_eq!(task_json["budget_secs"], 7);
}

#[tokio::test]
async fn test_dispatch_blocks_conflicting_write_sets() {
    let tmp = TempDir::new().unwrap();
    let state_dir = tmp.path().join("state");
    let event_log = state_dir.join(EVENTS_FILE);
    tokio::fs::create_dir_all(&state_dir).await.unwrap();

    let mut runner = TeamRunner::init_with_tasks(
        "run-test",
        tmp.path(),
        &state_dir,
        EventWriter::new(&event_log),
        vec![
            Task::new("task-1", "first writer")
                .with_description("write shared file first")
                .with_write_set(vec!["src/shared.rs".to_string()]),
            Task::new("task-2", "second writer")
                .with_description("write shared file second")
                .with_write_set(vec!["src/shared.rs".to_string()]),
        ],
    )
    .await
    .unwrap();

    let worker_a = make_spec(&tmp, "worker-a").await;
    let worker_b = make_spec(&tmp, "worker-b").await;
    runner
        .dispatch_to_workers(&[worker_a.clone(), worker_b.clone()])
        .await
        .unwrap();

    let task_1 = runner.claim_store.get(&"task-1".to_string()).unwrap();
    let task_2 = runner.claim_store.get(&"task-2".to_string()).unwrap();

    assert_eq!(task_1.state, TaskState::Running);
    assert_eq!(task_1.owner, Some("worker-a".to_string()));
    assert_eq!(task_2.state, TaskState::Pending);
    assert_eq!(task_2.owner, None);

    assert!(tokio::fs::read_to_string(&worker_a.inbox)
        .await
        .unwrap()
        .contains("task-1"));
    assert!(
        !worker_b.inbox.exists(),
        "conflicting task must not be dispatched to the second worker"
    );
}

#[tokio::test]
async fn test_poll_reads_worker_result_and_completes_task() {
    let tmp = TempDir::new().unwrap();
    let mut runner = make_runner(&tmp).await;
    runner.seed_task("Implement feature X");

    let spec = make_spec(&tmp, "worker-0").await;
    runner
        .dispatch_to_workers(std::slice::from_ref(&spec))
        .await
        .unwrap();

    let result = WorkerResult {
        task_id: "task-1".to_string(),
        status: ResultStatus::Success,
        summary: "done".to_string(),
        artifacts: vec![],
        elapsed_secs: 5,
    };
    let line = serde_json::to_string(&result).unwrap();
    tokio::fs::write(&spec.outbox, format!("{}\n", line))
        .await
        .unwrap();

    runner.poll_workers().await.unwrap();

    let task = runner.claim_store.get(&"task-1".to_string()).unwrap();
    assert_eq!(task.state, TaskState::Completed);
    assert!(task.completed_at.is_some());
}

#[tokio::test]
async fn test_poll_reads_simple_failed_result() {
    let tmp = TempDir::new().unwrap();
    let mut runner = make_runner(&tmp).await;
    runner.seed_task("Implement feature X");

    let spec = make_spec(&tmp, "worker-0").await;
    runner
        .dispatch_to_workers(std::slice::from_ref(&spec))
        .await
        .unwrap();

    let result = serde_json::json!({
        "id": "task-1",
        "status": "failed",
        "error": "compilation error",
    });
    tokio::fs::write(&spec.outbox, format!("{}\n", result))
        .await
        .unwrap();

    runner.poll_workers().await.unwrap();

    let task = runner.claim_store.get(&"task-1".to_string()).unwrap();
    assert_eq!(task.state, TaskState::Pending);
    assert_eq!(task.retry_count, 1);
}

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

    let task = runner.claim_store.get(&"task-1".to_string()).unwrap();
    assert_eq!(task.state, TaskState::Running);
    assert_eq!(task.owner, Some("worker-b".to_string()));
    assert!(
        tokio::fs::read_to_string(&worker_b.inbox)
            .await
            .unwrap()
            .contains("task-1"),
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

    let worker_a_inbox = tokio::fs::read_to_string(&worker_a.inbox).await.unwrap();
    assert!(
        !worker_a_inbox.contains("task-2"),
        "quarantined stale worker must not receive future tasks"
    );
    assert!(
        worker_a
            .inbox
            .parent()
            .unwrap()
            .join("stale-worker-cleanup.json")
            .exists(),
        "stale worker cleanup marker should be durable"
    );

    let events = EventReader::read_all(&tmp.path().join("state").join(EVENTS_FILE))
        .await
        .unwrap();
    assert!(events.iter().any(|event| {
        event.kind == EventKind::WorkerDead
            && event.actor.as_deref() == Some("scheduler")
            && event
                .payload
                .as_ref()
                .and_then(|payload| payload.get("worker_id"))
                .and_then(|value| value.as_str())
                == Some("worker-a")
    }));
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
    assert!(worker
        .inbox
        .parent()
        .unwrap()
        .join("stale-worker-cleanup.json")
        .exists());
}

#[tokio::test]
async fn test_run_with_cancel_reason_records_custom_task_failure_error() {
    let tmp = TempDir::new().unwrap();
    let mut runner = make_runner(&tmp).await;
    runner.seed_task("Pause this work");

    let worker = make_spec(&tmp, "worker-cancelled").await;
    let cancel = CancellationToken::new();
    cancel.cancel();

    let summary = runner
        .run_with_cancel_reason(std::slice::from_ref(&worker), &cancel, "cancelled by user")
        .await
        .unwrap();

    assert_eq!(summary.completed, 0);
    assert_eq!(summary.failed, 0);
    assert_eq!(summary.cancelled, 1);
    assert_eq!(summary.total, 1);

    let events = EventReader::read_all(&tmp.path().join("state").join(EVENTS_FILE))
        .await
        .unwrap();
    assert!(events.iter().any(|event| {
        event.kind == EventKind::TaskFailed
            && event
                .payload
                .as_ref()
                .and_then(|payload| payload.get("task_id"))
                .and_then(|value| value.as_str())
                == Some("task-1")
            && event
                .payload
                .as_ref()
                .and_then(|payload| payload.get("error"))
                .and_then(|value| value.as_str())
                == Some("cancelled by user")
    }));
}
