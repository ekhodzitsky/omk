use tempfile::TempDir;
use tokio::time::{timeout, Duration};

use crate::runtime::config::{EVENTS_FILE, WORKERS_DIR};
use crate::runtime::events::{EventKind, EventReader, EventWriter};
use crate::runtime::scheduler::runner::TeamRunner;
use crate::runtime::scheduler::task::{Task, TaskState};
use crate::runtime::worker::{ResultStatus, WorkerResult, WorkerSpec};
use tokio_util::sync::CancellationToken;

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

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

async fn make_runner_with_tasks(tmp: &TempDir, tasks: Vec<Task>) -> TeamRunner {
    let state_dir = tmp.path().join("state");
    let event_log = state_dir.join(EVENTS_FILE);
    tokio::fs::create_dir_all(&state_dir).await.unwrap();
    TeamRunner::init_with_tasks(
        "run-test",
        tmp.path(),
        &state_dir,
        EventWriter::new(&event_log),
        tasks,
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

async fn read_inbox(spec: &WorkerSpec) -> String {
    tokio::fs::read_to_string(&spec.inbox).await.unwrap()
}

async fn write_outbox(spec: &WorkerSpec, content: impl AsRef<[u8]>) {
    tokio::fs::write(&spec.outbox, content).await.unwrap();
}

fn assert_task_state(runner: &TeamRunner, task_id: &str, state: TaskState, owner: Option<&str>) {
    let task = runner
        .claim_store
        .get(&task_id.to_string())
        .expect("task must exist");
    assert_eq!(task.state, state, "task {task_id} state mismatch");
    assert_eq!(
        task.owner.as_deref(),
        owner,
        "task {task_id} owner mismatch"
    );
}

fn assert_task_completed(runner: &TeamRunner, task_id: &str) {
    let task = runner
        .claim_store
        .get(&task_id.to_string())
        .expect("task must exist");
    assert_eq!(task.state, TaskState::Completed, "task {task_id} must be completed");
    assert!(task.completed_at.is_some(), "task {task_id} must have completed_at");
}

fn success_result(task_id: &str) -> WorkerResult {
    WorkerResult {
        task_id: task_id.to_string(),
        status: ResultStatus::Success,
        summary: "done".to_string(),
        artifacts: vec![],
        elapsed_secs: 5,
    }
}

fn simple_failed_json(task_id: &str, error: &str) -> serde_json::Value {
    serde_json::json!({
        "id": task_id,
        "status": "failed",
        "error": error,
    })
}

fn stale_cleanup_marker(tmp: &TempDir, worker_name: &str) -> std::path::PathBuf {
    tmp.path()
        .join("state")
        .join(WORKERS_DIR)
        .join(worker_name)
        .join("stale-worker-cleanup.json")
}

async fn assert_scheduler_event_present(tmp: &TempDir, kind: EventKind, worker_id: &str) {
    let events = EventReader::read_all(&tmp.path().join("state").join(EVENTS_FILE))
        .await
        .unwrap();
    assert!(
        events.iter().any(|event| {
            event.kind == kind
                && event.actor.as_deref() == Some("scheduler")
                && event
                    .payload
                    .as_ref()
                    .and_then(|payload| payload.get("worker_id"))
                    .and_then(|value| value.as_str())
                    == Some(worker_id)
        }),
        "expected {kind:?} event for worker {worker_id}"
    );
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

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

    let inbox = read_inbox(&spec).await;
    assert!(inbox.contains("task-1"));
    assert!(inbox.contains("Implement feature X"));

    assert_task_state(&runner, "task-1", TaskState::Running, Some("worker-0"));
}

#[tokio::test]
async fn test_dispatch_includes_structured_task_budget_in_inbox() {
    let tmp = TempDir::new().unwrap();
    let mut task =
        Task::new("task-budgeted", "budgeted task").with_description("honor this task budget");
    task.extra
        .insert("budget_secs".to_string(), serde_json::json!(7));

    let mut runner = make_runner_with_tasks(&tmp, vec![task]).await;

    let spec = make_spec(&tmp, "worker-budget").await;
    runner
        .dispatch_to_workers(std::slice::from_ref(&spec))
        .await
        .unwrap();

    let inbox = read_inbox(&spec).await;
    let task_json: serde_json::Value = serde_json::from_str(inbox.lines().next().unwrap()).unwrap();
    assert_eq!(task_json["id"], "task-budgeted");
    assert_eq!(task_json["budget_secs"], 7);
}

#[tokio::test]
async fn test_dispatch_blocks_conflicting_write_sets() {
    let tmp = TempDir::new().unwrap();
    let mut runner = make_runner_with_tasks(
        &tmp,
        vec![
            Task::new("task-1", "first writer")
                .with_description("write shared file first")
                .with_write_set(vec!["src/shared.rs".to_string()]),
            Task::new("task-2", "second writer")
                .with_description("write shared file second")
                .with_write_set(vec!["src/shared.rs".to_string()]),
        ],
    )
    .await;

    let worker_a = make_spec(&tmp, "worker-a").await;
    let worker_b = make_spec(&tmp, "worker-b").await;
    runner
        .dispatch_to_workers(&[worker_a.clone(), worker_b.clone()])
        .await
        .unwrap();

    assert_task_state(&runner, "task-1", TaskState::Running, Some("worker-a"));
    assert_task_state(&runner, "task-2", TaskState::Pending, None);

    let inbox_a = read_inbox(&worker_a).await;
    assert!(inbox_a.contains("task-1"));
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

    let result = success_result("task-1");
    write_outbox(
        &spec,
        format!("{}\n", serde_json::to_string(&result).unwrap()),
    )
    .await;

    runner.poll_workers().await.unwrap();

    assert_task_completed(&runner, "task-1");
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

    let result = simple_failed_json("task-1", "compilation error");
    write_outbox(&spec, format!("{}\n", result)).await;

    runner.poll_workers().await.unwrap();

    assert_task_state(&runner, "task-1", TaskState::Pending, None);
    assert_eq!(
        runner
            .claim_store
            .get(&"task-1".to_string())
            .unwrap()
            .retry_count,
        1
    );
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
