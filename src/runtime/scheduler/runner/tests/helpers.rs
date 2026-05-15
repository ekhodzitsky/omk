use tempfile::TempDir;

use crate::runtime::config::{EVENTS_FILE, WORKERS_DIR};
use crate::runtime::events::{EventKind, EventReader, EventWriter};
use crate::runtime::scheduler::runner::TeamRunner;
use crate::runtime::scheduler::task::{Task, TaskState};
use crate::runtime::worker::{ResultStatus, WorkerResult, WorkerSpec};

pub async fn make_runner(tmp: &TempDir) -> TeamRunner {
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

pub async fn make_runner_with_tasks(tmp: &TempDir, tasks: Vec<Task>) -> TeamRunner {
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

pub async fn make_spec(tmp: &TempDir, name: &str) -> WorkerSpec {
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

pub async fn read_inbox(spec: &WorkerSpec) -> String {
    tokio::fs::read_to_string(&spec.inbox).await.unwrap()
}

pub async fn write_outbox(spec: &WorkerSpec, content: impl AsRef<[u8]>) {
    tokio::fs::write(&spec.outbox, content).await.unwrap();
}

pub fn assert_task_state(
    runner: &TeamRunner,
    task_id: &str,
    state: TaskState,
    owner: Option<&str>,
) {
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

pub fn assert_task_completed(runner: &TeamRunner, task_id: &str) {
    let task = runner
        .claim_store
        .get(&task_id.to_string())
        .expect("task must exist");
    assert_eq!(
        task.state,
        TaskState::Completed,
        "task {task_id} must be completed"
    );
    assert!(
        task.completed_at.is_some(),
        "task {task_id} must have completed_at"
    );
}

pub fn success_result(task_id: &str) -> WorkerResult {
    WorkerResult {
        task_id: task_id.to_string(),
        status: ResultStatus::Success,
        summary: "done".to_string(),
        artifacts: vec![],
        elapsed_secs: 5,
    }
}

pub fn simple_failed_json(task_id: &str, error: &str) -> serde_json::Value {
    serde_json::json!({
        "id": task_id,
        "status": "failed",
        "error": error,
    })
}

pub fn stale_cleanup_marker(tmp: &TempDir, worker_name: &str) -> std::path::PathBuf {
    tmp.path()
        .join("state")
        .join(WORKERS_DIR)
        .join(worker_name)
        .join("stale-worker-cleanup.json")
}

pub async fn assert_scheduler_event_present(tmp: &TempDir, kind: EventKind, worker_id: &str) {
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
