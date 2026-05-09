use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::io::{AsyncBufReadExt, AsyncSeekExt};

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use tracing::{info, warn};

#[cfg(test)]
use crate::runtime::config::EVENTS_FILE;
use crate::runtime::config::{HEARTBEAT_FILE, OUTBOX_FILE, WORKERS_DIR};
use crate::runtime::events::{
    Event, EventBuilder, EventKind, EventWriter, RunId, TaskId, WorkerId,
};
use crate::runtime::scheduler::claim::ClaimStore;
use crate::runtime::scheduler::manifest::RunManifest;
use crate::runtime::scheduler::ownership::OwnershipMap;
use crate::runtime::scheduler::task::{Task, TaskState};
use crate::runtime::worker::{ResultStatus, WorkerResult, WorkerSpec};

/// Poll interval for the runner dispatch loop.
pub const RUNNER_POLL_INTERVAL_SECS: u64 = 1;

/// Orchestrates a team run using the scheduler: claims tasks, dispatches to
/// workers via inbox/outbox, and drives the run to completion.
pub struct TeamRunner {
    pub(crate) manifest: RunManifest,
    pub(crate) claim_store: ClaimStore,
    pub(crate) ownership: OwnershipMap,
    event_writer: EventWriter,
    state_dir: PathBuf,
    run_id: RunId,
    last_outbox_offsets: HashMap<String, u64>,
    last_heartbeat_ts: HashMap<String, DateTime<Utc>>,
}

/// Summary of a completed (or failed) team run.
#[derive(Debug, Clone)]
pub struct RunSummary {
    pub run_id: String,
    pub completed: usize,
    pub failed: usize,
    pub cancelled: usize,
    pub total: usize,
}

impl TeamRunner {
    /// Initialize a new team run.
    pub async fn init(
        run_id: &str,
        task_desc: &str,
        project_dir: &Path,
        state_dir: &Path,
        event_writer: EventWriter,
    ) -> Result<Self> {
        let manifest = RunManifest::new(run_id, "team", project_dir).with_description(task_desc);
        manifest.init().await?;

        Ok(Self {
            manifest,
            claim_store: ClaimStore::new(),
            ownership: OwnershipMap::new(),
            event_writer,
            state_dir: state_dir.to_path_buf(),
            run_id: RunId(run_id.to_string()),
            last_outbox_offsets: HashMap::new(),
            last_heartbeat_ts: HashMap::new(),
        })
    }

    /// Initialize a new team run with a pre-built task list.
    pub async fn init_with_tasks(
        run_id: &str,
        project_dir: &Path,
        state_dir: &Path,
        event_writer: EventWriter,
        tasks: Vec<Task>,
    ) -> Result<Self> {
        let manifest = RunManifest::new(run_id, "team", project_dir).with_tasks(tasks.clone());
        manifest.init().await?;

        let mut claim_store = ClaimStore::new();
        for task in &tasks {
            claim_store.insert(task.clone());
        }

        Ok(Self {
            manifest,
            claim_store,
            ownership: OwnershipMap::new(),
            event_writer,
            state_dir: state_dir.to_path_buf(),
            run_id: RunId(run_id.to_string()),
            last_outbox_offsets: HashMap::new(),
            last_heartbeat_ts: HashMap::new(),
        })
    }

    /// Create a single seed task from a high-level description.
    pub fn seed_task(&mut self, description: &str) {
        let task = Task::new("task-1", "seed").with_description(description);
        self.claim_store.insert(task.clone());
        self.manifest.tasks.push(task);
    }

    /// Create multiple seed tasks from (task_id, description) pairs.
    pub fn seed_tasks(&mut self, descriptions: Vec<(String, String)>) {
        for (task_id, description) in descriptions {
            let task = Task::new(&task_id, "seed").with_description(&description);
            self.claim_store.insert(task.clone());
            self.manifest.tasks.push(task);
        }
    }

    /// Assign ready tasks to available workers.
    pub async fn dispatch_to_workers(&mut self, worker_specs: &[WorkerSpec]) -> Result<()> {
        // Workers that currently own a claimed or running task are busy.
        let busy_workers: std::collections::HashSet<String> = self
            .claim_store
            .tasks()
            .values()
            .filter(|t| matches!(t.state, TaskState::Claimed | TaskState::Running))
            .filter_map(|t| t.owner.clone())
            .collect();

        let mut available_workers: Vec<&WorkerSpec> = worker_specs
            .iter()
            .filter(|w| !busy_workers.contains(&w.name))
            .collect();

        if available_workers.is_empty() {
            return Ok(());
        }

        // Collect ready task IDs so we can release the immutable borrow on claim_store.
        let ready_ids: Vec<String> = {
            let ready = self.claim_store.ready_tasks();
            ready.iter().map(|t| t.id.clone()).collect()
        };

        for task_id in ready_ids {
            if available_workers.is_empty() {
                break;
            }

            let task = match self.claim_store.get(&task_id) {
                Some(t) => t.clone(),
                None => continue,
            };

            // Find first worker that doesn't have an ownership conflict.
            let mut assigned_idx = None;
            for (idx, worker) in available_workers.iter().enumerate() {
                let conflicts = self.ownership.would_conflict(&task, &worker.name);
                if conflicts.is_empty() {
                    assigned_idx = Some(idx);
                    break;
                }
                warn!(
                    task = %task.id,
                    worker = %worker.name,
                    conflicts = ?conflicts,
                    "Ownership conflict detected"
                );
            }

            let Some(idx) = assigned_idx else {
                warn!(
                    task = %task.id,
                    write_set = ?task.write_set,
                    "Task dispatch blocked by active ownership conflicts"
                );
                continue;
            };
            let worker = available_workers.remove(idx);

            if !self.claim_store.claim(&task_id, &worker.name) {
                continue;
            }

            // Register ownership for the task's write set.
            if let Some(task_ref) = self.claim_store.get(&task_id) {
                self.ownership.register_task(task_ref);
            }

            // Emit TaskClaimed event.
            let lease_secs = self
                .claim_store
                .tasks()
                .get(&task_id)
                .and_then(|t| t.lease_expires)
                .map(|dt| dt.timestamp() as u64)
                .unwrap_or(300);
            let claimed_event = EventBuilder::new(self.run_id.clone()).task_claimed(
                TaskId(task_id.clone()),
                WorkerId(worker.name.clone()),
                lease_secs,
            )?;
            self.event_writer.append(&claimed_event).await?;

            // Emit TaskStarted event (scaffold: treat dispatch as start).
            let started_event = Event::new(self.run_id.clone(), EventKind::TaskStarted)
                .with_actor(&worker.name)
                .with_payload(serde_json::json!({
                    "task_id": task_id,
                    "worker_id": worker.name,
                }))?;
            self.event_writer.append(&started_event).await?;
            self.claim_store.start(&task_id, &worker.name);

            // Write task to worker inbox.
            let worker_task = crate::runtime::worker::WorkerTask {
                id: task_id.clone(),
                task: task.description.clone(),
                acceptance_criteria: Vec::new(),
                context: None,
            };
            worker.send_task(&worker_task).await?;

            info!(task = %task_id, worker = %worker.name, "Dispatched task to worker");
        }

        Ok(())
    }

    /// Poll worker outboxes and update the claim store.
    pub async fn poll_workers(&mut self) -> Result<()> {
        let workers_dir = self.state_dir.join(WORKERS_DIR);
        if !workers_dir.exists() {
            return Ok(());
        }

        let mut entries = tokio::fs::read_dir(&workers_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let worker_dir = entry.path();
            let outbox = worker_dir.join(OUTBOX_FILE);
            if !outbox.exists() {
                continue;
            }

            let worker_name = worker_dir
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();

            let file = tokio::fs::OpenOptions::new()
                .read(true)
                .open(&outbox)
                .await?;
            let mut reader = tokio::io::BufReader::new(file);
            let metadata = reader.get_ref().metadata().await?;
            let file_len = metadata.len();

            let last_offset = *self.last_outbox_offsets.get(&worker_name).unwrap_or(&0);
            if file_len < last_offset {
                self.last_outbox_offsets.insert(worker_name.clone(), 0);
            }

            reader.seek(tokio::io::SeekFrom::Start(last_offset)).await?;

            let mut line = String::new();
            loop {
                line.clear();
                let bytes_read = reader.read_line(&mut line).await?;
                if bytes_read == 0 {
                    break;
                }
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                self.process_outbox_line(&worker_name, trimmed).await?;
            }

            let new_offset = reader.stream_position().await?;
            self.last_outbox_offsets
                .insert(worker_name.clone(), new_offset);

            // Check for fresh heartbeats and emit events.
            let heartbeat = worker_dir.join(HEARTBEAT_FILE);
            if heartbeat.exists() {
                if let Ok(content) = tokio::fs::read_to_string(&heartbeat).await {
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                        if let Some(ts_str) = json.get("ts").and_then(|v| v.as_str()) {
                            if let Ok(ts) = ts_str.parse::<DateTime<Utc>>() {
                                let last = self.last_heartbeat_ts.get(&worker_name).copied();
                                if last.map_or(true, |l| ts > l) {
                                    let event = EventBuilder::new(self.run_id.clone())
                                        .worker_heartbeat(WorkerId(worker_name.clone()))?;
                                    self.event_writer.append(&event).await?;
                                    self.last_heartbeat_ts.insert(worker_name, ts);
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    async fn process_outbox_line(&mut self, worker_name: &str, line: &str) -> Result<()> {
        let parsed: ParsedResult = match serde_json::from_str::<WorkerResult>(line) {
            Ok(r) => ParsedResult {
                task_id: r.task_id,
                status: match r.status {
                    ResultStatus::Success | ResultStatus::Partial => "completed".to_string(),
                    ResultStatus::Failed => "failed".to_string(),
                },
                summary: r.summary,
                error: String::new(),
            },
            Err(_) => match serde_json::from_str::<SimpleResult>(line) {
                Ok(r) => ParsedResult {
                    task_id: r.id,
                    status: r.status,
                    summary: r.result.unwrap_or_default(),
                    error: r.error.unwrap_or_default(),
                },
                Err(e) => {
                    warn!(line = %line, error = %e, "Failed to parse outbox line");
                    return Ok(());
                }
            },
        };

        match parsed.status.as_str() {
            "completed" | "success" => {
                if self.claim_store.complete(&parsed.task_id, worker_name) {
                    if let Some(task) = self.claim_store.get(&parsed.task_id) {
                        self.ownership.release_task(task);
                    }
                    let event = EventBuilder::new(self.run_id.clone()).task_completed(
                        TaskId(parsed.task_id.clone()),
                        WorkerId(worker_name.to_string()),
                        Some(&parsed.summary),
                    )?;
                    self.event_writer.append(&event).await?;
                    info!(task = %parsed.task_id, worker = %worker_name, "Task completed");
                }
            }
            "failed" => {
                if self.claim_store.fail(&parsed.task_id, worker_name) {
                    if let Some(task) = self.claim_store.get(&parsed.task_id) {
                        self.ownership.release_task(task);
                    }
                    let event = Event::new(self.run_id.clone(), EventKind::TaskFailed)
                        .with_actor(worker_name)
                        .with_payload(serde_json::json!({
                            "task_id": parsed.task_id,
                            "worker_id": worker_name,
                            "error": parsed.error,
                        }))?;
                    self.event_writer.append(&event).await?;
                    info!(task = %parsed.task_id, worker = %worker_name, "Task failed");
                }
            }
            _ => {
                warn!(status = %parsed.status, "Unknown result status in outbox");
            }
        }

        Ok(())
    }

    /// Run the main loop until all tasks are done.
    pub async fn run(&mut self, worker_specs: &[WorkerSpec]) -> Result<RunSummary> {
        loop {
            self.dispatch_to_workers(worker_specs).await?;
            self.poll_workers().await?;

            let recovered = self.claim_store.recover_stale_leases();
            for task_id in &recovered {
                if let Some(task) = self.claim_store.get(task_id) {
                    self.ownership.release_task(task);
                }
                let event = Event::new(self.run_id.clone(), EventKind::RetryScheduled)
                    .with_actor("scheduler")
                    .with_message(format!("stale lease recovered for {}", task_id))?;
                self.event_writer.append(&event).await?;
            }

            self.snapshot().await?;

            if self.claim_store.all_done() {
                break;
            }

            tokio::time::sleep(std::time::Duration::from_secs(RUNNER_POLL_INTERVAL_SECS)).await;
        }

        let summary = self.claim_store.summary();
        let success = summary.failed == 0 && summary.cancelled == 0;

        if success {
            let event = EventBuilder::new(self.run_id.clone()).run_completed();
            self.event_writer.append(&event).await?;
        } else {
            let event = EventBuilder::new(self.run_id.clone())
                .run_failed("one or more tasks failed or were cancelled")?;
            self.event_writer.append(&event).await?;
        }

        Ok(RunSummary {
            run_id: self.run_id.0.clone(),
            completed: summary.completed,
            failed: summary.failed,
            cancelled: summary.cancelled,
            total: summary.total(),
        })
    }

    /// Save current state to manifest.
    pub async fn snapshot(&self) -> Result<()> {
        let mut manifest = self.manifest.clone();
        manifest.tasks = self.claim_store.tasks().values().cloned().collect();
        manifest.save().await?;
        manifest.snapshot_tasks().await?;
        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize)]
struct SimpleResult {
    id: String,
    status: String,
    #[serde(default)]
    result: Option<String>,
    #[serde(default)]
    error: Option<String>,
}

struct ParsedResult {
    task_id: String,
    status: String,
    summary: String,
    error: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

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

        // Verify inbox contains the task assignment.
        let inbox = tokio::fs::read_to_string(&spec.inbox).await.unwrap();
        assert!(inbox.contains("task-1"));
        assert!(inbox.contains("Implement feature X"));

        // Verify claim store state.
        let task = runner.claim_store.get(&"task-1".to_string()).unwrap();
        assert_eq!(task.state, TaskState::Running); // claimed + started
        assert_eq!(task.owner, Some("worker-0".to_string()));
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

        // Simulate a worker result in the outbox (bridge format).
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

        // Simulate a simple failure result.
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
        assert_eq!(task.state, TaskState::Pending); // retry scheduled
        assert_eq!(task.retry_count, 1);
    }
}
