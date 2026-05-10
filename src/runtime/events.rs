use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tracing::{debug, warn};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Newtypes
// ---------------------------------------------------------------------------

/// Unique identifier for a run.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RunId(pub String);

impl RunId {
    pub fn generate() -> Self {
        Self(format!("run-{}", Utc::now().format("%Y%m%d-%H%M%S-%3f")))
    }
}

impl std::fmt::Display for RunId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Unique identifier for a single event within a run.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EventId(pub String);

impl EventId {
    pub fn generate() -> Self {
        Self(Uuid::new_v4().to_string())
    }
}

impl std::fmt::Display for EventId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Unique identifier for a worker within a run.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WorkerId(pub String);

impl std::fmt::Display for WorkerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Unique identifier for a task within a run.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TaskId(pub String);

impl std::fmt::Display for TaskId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Unique identifier for a verification gate.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GateId(pub String);

impl std::fmt::Display for GateId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ---------------------------------------------------------------------------
// Event envelope
// ---------------------------------------------------------------------------

/// Current event schema version. Bumped when the envelope shape changes.
pub const EVENT_SCHEMA_VERSION: u32 = 1;

/// A single event in the append-only event log.
///
/// Every event carries a common envelope plus a payload that depends on `kind`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub id: EventId,
    pub run_id: RunId,
    pub ts: DateTime<Utc>,
    pub schema_version: u32,
    pub kind: EventKind,
    pub actor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<serde_json::Value>,
}

impl Event {
    pub fn new(run_id: RunId, kind: EventKind) -> Self {
        Self {
            id: EventId::generate(),
            run_id,
            ts: Utc::now(),
            schema_version: EVENT_SCHEMA_VERSION,
            kind,
            actor: None,
            payload: None,
        }
    }

    pub fn with_actor(mut self, actor: impl Into<String>) -> Self {
        self.actor = Some(actor.into());
        self
    }

    pub fn with_message(mut self, message: impl Into<String>) -> Result<Self> {
        self.payload = Some(serde_json::json!({ "message": message.into() }));
        Ok(self)
    }

    pub fn with_payload(mut self, payload: impl Serialize) -> Result<Self> {
        self.payload = Some(serde_json::to_value(payload)?);
        Ok(self)
    }
}

// ---------------------------------------------------------------------------
// Event kinds
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    RunStarted,
    RunCompleted,
    RunFailed,
    WorkerStarted,
    WorkerHeartbeat,
    WorkerStalled,
    WorkerDead,
    WorkerRecovered,
    TaskClaimed,
    TaskStarted,
    TaskOutput,
    TaskCompleted,
    TaskFailed,
    FileChanged,
    CommandStarted,
    CommandFinished,
    GatePassed,
    GateFailed,
    RetryScheduled,
    ProofWritten,
    ManualInterrupt,
}

// ---------------------------------------------------------------------------
// Typed payloads (optional helpers)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunStartedPayload {
    pub mode: String,
    pub project_dir: PathBuf,
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kimi_binary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kimi_cli_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wire_protocol_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerStartedPayload {
    pub worker_id: WorkerId,
    pub role: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerHeartbeatPayload {
    pub worker_id: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskClaimedPayload {
    pub task_id: TaskId,
    pub worker_id: WorkerId,
    pub lease_deadline: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskCompletedPayload {
    pub task_id: TaskId,
    pub worker_id: WorkerId,
    pub output_summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChangedPayload {
    pub path: String,
    pub operation: String, // "created", "modified", "deleted"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandStartedPayload {
    pub gate_id: GateId,
    pub name: String,
    pub command_line: String,
    pub timeout_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandFinishedPayload {
    pub gate_id: GateId,
    pub name: String,
    #[serde(alias = "command")]
    pub command_line: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    #[serde(default)]
    pub timed_out: bool,
    pub stdout_summary: Option<String>,
    pub stderr_summary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateResultPayload {
    pub gate_id: GateId,
    pub name: String,
    pub required: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command_line: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    #[serde(default)]
    pub timed_out: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stdout_summary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stderr_summary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_secs: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProofWrittenPayload {
    pub proof_path: PathBuf,
    pub status: String, // "ready", "not_ready", "failed"
}

// ---------------------------------------------------------------------------
// Writer
// ---------------------------------------------------------------------------

/// Append-only JSONL event writer.
#[derive(Clone)]
pub struct EventWriter {
    path: PathBuf,
}

impl EventWriter {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Append a single event atomically-ish: open, write, flush, close.
    /// This is not OS-level atomic, but it minimizes the window for corruption.
    pub async fn append(&self, event: &Event) -> Result<()> {
        let mut line = serde_json::to_vec(event)?;
        line.push(b'\n');
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .await?;
        use tokio::io::AsyncWriteExt;
        file.write_all(&line).await?;
        file.flush().await?;
        debug!(event_id = %event.id, "Appended event");
        Ok(())
    }

    pub async fn append_many(&self, events: &[Event]) -> Result<()> {
        let mut buffer = Vec::new();
        for event in events {
            serde_json::to_writer(&mut buffer, event)?;
            buffer.push(b'\n');
        }

        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .await?;
        use tokio::io::AsyncWriteExt;
        file.write_all(&buffer).await?;
        file.flush().await?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Reader
// ---------------------------------------------------------------------------

/// Event reader that tolerates partial or corrupt trailing lines.
pub struct EventReader;

impl EventReader {
    /// Read all valid events from a JSONL file.
    /// Skips lines that fail to parse and logs a warning for each.
    pub async fn read_all(path: &Path) -> Result<Vec<Event>> {
        let content = match tokio::fs::read_to_string(path).await {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e.into()),
        };

        let mut events = Vec::new();
        for (line_no, line) in content.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            match serde_json::from_str::<Event>(line) {
                Ok(event) => events.push(event),
                Err(e) => {
                    warn!(line = line_no + 1, error = %e, "Skipping malformed event line");
                }
            }
        }
        Ok(events)
    }

    /// Read events filtered by kind.
    pub async fn read_filtered(path: &Path, kinds: &[EventKind]) -> Result<Vec<Event>> {
        let all = Self::read_all(path).await?;
        let filtered: Vec<_> = all
            .into_iter()
            .filter(|e| kinds.contains(&e.kind))
            .collect();
        Ok(filtered)
    }

    /// Read events for a specific worker.
    pub async fn read_for_worker(path: &Path, worker: &str) -> Result<Vec<Event>> {
        let all = Self::read_all(path).await?;
        let filtered: Vec<_> = all
            .into_iter()
            .filter(|e| e.actor.as_deref() == Some(worker))
            .collect();
        Ok(filtered)
    }

    /// Read events for a specific task id.
    pub async fn read_for_task(path: &Path, task_id: &str) -> Result<Vec<Event>> {
        let all = Self::read_all(path).await?;
        let filtered: Vec<_> = all
            .into_iter()
            .filter(|e| payload_string(e, "task_id").as_deref() == Some(task_id))
            .collect();
        Ok(filtered)
    }

    /// Read events for a specific gate id or gate name.
    pub async fn read_for_gate(path: &Path, gate: &str) -> Result<Vec<Event>> {
        let all = Self::read_all(path).await?;
        let filtered: Vec<_> = all
            .into_iter()
            .filter(|e| {
                payload_string(e, "gate_id").as_deref() == Some(gate)
                    || payload_string(e, "name").as_deref() == Some(gate)
            })
            .collect();
        Ok(filtered)
    }

    /// Read events within a time range.
    pub async fn read_range(
        path: &Path,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<Event>> {
        let all = Self::read_all(path).await?;
        let filtered: Vec<_> = all
            .into_iter()
            .filter(|e| e.ts >= from && e.ts <= to)
            .collect();
        Ok(filtered)
    }

    /// Return a summary: total lines, valid events, parse failures.
    pub async fn summary(path: &Path) -> Result<EventLogSummary> {
        let content = match tokio::fs::read_to_string(path).await {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(EventLogSummary::default())
            }
            Err(e) => return Err(e.into()),
        };

        let mut summary = EventLogSummary {
            total_lines: content.lines().count(),
            ..Default::default()
        };

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() {
                summary.empty_lines += 1;
                continue;
            }
            match serde_json::from_str::<Event>(line) {
                Ok(_) => summary.valid_events += 1,
                Err(_) => summary.parse_failures += 1,
            }
        }
        Ok(summary)
    }
}

fn payload_string(event: &Event, key: &str) -> Option<String> {
    event.payload.as_ref()?.get(key).and_then(|value| {
        if let Some(text) = value.as_str() {
            Some(text.to_string())
        } else {
            value
                .get("0")
                .and_then(|inner| inner.as_str())
                .map(str::to_string)
        }
    })
}

#[derive(Debug, Clone, Default)]
pub struct EventLogSummary {
    pub total_lines: usize,
    pub valid_events: usize,
    pub parse_failures: usize,
    pub empty_lines: usize,
}

// ---------------------------------------------------------------------------
// Event builder helpers
// ---------------------------------------------------------------------------

/// Convenience builder for common event patterns.
pub struct EventBuilder {
    run_id: RunId,
}

impl EventBuilder {
    pub fn new(run_id: RunId) -> Self {
        Self { run_id }
    }

    pub fn run_started(&self, mode: &str, project_dir: &Path, description: &str) -> Result<Event> {
        self.run_started_with_kimi_metadata(mode, project_dir, description, None, None, None)
    }

    pub fn run_started_with_kimi_metadata(
        &self,
        mode: &str,
        project_dir: &Path,
        description: &str,
        kimi_binary: Option<String>,
        kimi_cli_version: Option<String>,
        wire_protocol_version: Option<String>,
    ) -> Result<Event> {
        Event::new(self.run_id.clone(), EventKind::RunStarted).with_payload(RunStartedPayload {
            mode: mode.to_string(),
            project_dir: project_dir.to_path_buf(),
            description: description.to_string(),
            kimi_binary,
            kimi_cli_version,
            wire_protocol_version,
        })
    }

    pub fn worker_started(&self, worker_id: WorkerId, role: &str) -> Result<Event> {
        Event::new(self.run_id.clone(), EventKind::WorkerStarted)
            .with_actor(worker_id.0.clone())
            .with_payload(WorkerStartedPayload {
                worker_id,
                role: role.to_string(),
            })
    }

    pub fn worker_heartbeat(&self, worker_id: WorkerId) -> Result<Event> {
        Event::new(self.run_id.clone(), EventKind::WorkerHeartbeat)
            .with_actor(worker_id.0.clone())
            .with_payload(WorkerHeartbeatPayload {
                worker_id: worker_id.0,
                timestamp: Utc::now(),
            })
    }

    pub fn task_claimed(
        &self,
        task_id: TaskId,
        worker_id: WorkerId,
        lease_secs: u64,
    ) -> Result<Event> {
        Event::new(self.run_id.clone(), EventKind::TaskClaimed)
            .with_actor(worker_id.0.clone())
            .with_payload(TaskClaimedPayload {
                task_id,
                worker_id,
                lease_deadline: Utc::now() + chrono::Duration::seconds(lease_secs as i64),
            })
    }

    pub fn task_completed(
        &self,
        task_id: TaskId,
        worker_id: WorkerId,
        output_summary: Option<&str>,
    ) -> Result<Event> {
        Event::new(self.run_id.clone(), EventKind::TaskCompleted)
            .with_actor(worker_id.0.clone())
            .with_payload(TaskCompletedPayload {
                task_id,
                worker_id,
                output_summary: output_summary.map(|s| s.to_string()),
            })
    }

    pub fn file_changed(&self, path: &str, operation: &str) -> Result<Event> {
        Event::new(self.run_id.clone(), EventKind::FileChanged).with_payload(FileChangedPayload {
            path: path.to_string(),
            operation: operation.to_string(),
        })
    }

    pub fn command_started(
        &self,
        gate_id: GateId,
        name: &str,
        command_line: &str,
        timeout_secs: u64,
    ) -> Result<Event> {
        Event::new(self.run_id.clone(), EventKind::CommandStarted).with_payload(
            CommandStartedPayload {
                gate_id,
                name: name.to_string(),
                command_line: command_line.to_string(),
                timeout_secs,
            },
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn command_finished(
        &self,
        gate_id: GateId,
        name: &str,
        command_line: &str,
        exit_code: Option<i32>,
        timed_out: bool,
        stdout_summary: Option<&str>,
        stderr_summary: Option<&str>,
        output_path: Option<&str>,
    ) -> Result<Event> {
        Event::new(self.run_id.clone(), EventKind::CommandFinished).with_payload(
            CommandFinishedPayload {
                gate_id,
                name: name.to_string(),
                command_line: command_line.to_string(),
                exit_code,
                timed_out,
                stdout_summary: stdout_summary.map(str::to_string),
                stderr_summary: stderr_summary.map(str::to_string),
                output_path: output_path.map(str::to_string),
            },
        )
    }

    pub fn gate_passed(&self, gate_id: GateId, name: &str, required: bool) -> Result<Event> {
        self.gate_passed_with_evidence(
            gate_id, name, required, None, None, false, None, None, None, None,
        )
    }

    pub fn gate_failed(&self, gate_id: GateId, name: &str, required: bool) -> Result<Event> {
        self.gate_failed_with_evidence(
            gate_id, name, required, None, None, false, None, None, None, None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn gate_passed_with_evidence(
        &self,
        gate_id: GateId,
        name: &str,
        required: bool,
        command_line: Option<&str>,
        exit_code: Option<i32>,
        timed_out: bool,
        stdout_summary: Option<&str>,
        stderr_summary: Option<&str>,
        output_path: Option<&str>,
        timeout_secs: Option<u64>,
    ) -> Result<Event> {
        Event::new(self.run_id.clone(), EventKind::GatePassed).with_payload(GateResultPayload {
            gate_id,
            name: name.to_string(),
            required,
            command_line: command_line.map(str::to_string),
            exit_code,
            timed_out,
            stdout_summary: stdout_summary.map(str::to_string),
            stderr_summary: stderr_summary.map(str::to_string),
            output_path: output_path.map(str::to_string),
            timeout_secs,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn gate_failed_with_evidence(
        &self,
        gate_id: GateId,
        name: &str,
        required: bool,
        command_line: Option<&str>,
        exit_code: Option<i32>,
        timed_out: bool,
        stdout_summary: Option<&str>,
        stderr_summary: Option<&str>,
        output_path: Option<&str>,
        timeout_secs: Option<u64>,
    ) -> Result<Event> {
        Event::new(self.run_id.clone(), EventKind::GateFailed).with_payload(GateResultPayload {
            gate_id,
            name: name.to_string(),
            required,
            command_line: command_line.map(str::to_string),
            exit_code,
            timed_out,
            stdout_summary: stdout_summary.map(str::to_string),
            stderr_summary: stderr_summary.map(str::to_string),
            output_path: output_path.map(str::to_string),
            timeout_secs,
        })
    }

    /// Convenience method for creating a gate-passed event by name only.
    pub fn gate_passed_by_name(&self, name: &str) -> Result<Event> {
        self.gate_passed(GateId(name.to_string()), name, true)
    }

    /// Convenience method for creating a gate-failed event by name only.
    pub fn gate_failed_by_name(&self, name: &str) -> Result<Event> {
        self.gate_failed(GateId(name.to_string()), name, true)
    }

    pub fn proof_written(&self, proof_path: &Path, status: &str) -> Result<Event> {
        Event::new(self.run_id.clone(), EventKind::ProofWritten).with_payload(ProofWrittenPayload {
            proof_path: proof_path.to_path_buf(),
            status: status.to_string(),
        })
    }

    pub fn run_completed(&self) -> Event {
        Event::new(self.run_id.clone(), EventKind::RunCompleted)
    }

    pub fn run_failed(&self, reason: &str) -> Result<Event> {
        Event::new(self.run_id.clone(), EventKind::RunFailed)
            .with_payload(serde_json::json!({ "reason": reason }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn event_roundtrip() {
        let event = Event::new(RunId("test".to_string()), EventKind::RunStarted)
            .with_actor("worker-a")
            .with_payload(RunStartedPayload {
                mode: "team".to_string(),
                project_dir: PathBuf::from("/tmp/test"),
                description: "test run".to_string(),
                kimi_binary: None,
                kimi_cli_version: None,
                wire_protocol_version: None,
            })
            .unwrap();

        let json = serde_json::to_string(&event).unwrap();
        let restored: Event = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.run_id.0, "test");
        assert_eq!(restored.actor, Some("worker-a".to_string()));
        assert_eq!(restored.schema_version, EVENT_SCHEMA_VERSION);
    }

    #[tokio::test]
    async fn writer_reader_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("events.jsonl");

        let writer = EventWriter::new(&path);
        let run_id = RunId("run-1".to_string());

        let e1 = Event::new(run_id.clone(), EventKind::RunStarted);
        let e2 = Event::new(run_id.clone(), EventKind::WorkerStarted).with_actor("w1");
        let e3 = Event::new(run_id.clone(), EventKind::RunCompleted);

        writer.append(&e1).await.unwrap();
        writer.append(&e2).await.unwrap();
        writer.append(&e3).await.unwrap();

        let events = EventReader::read_all(&path).await.unwrap();
        assert_eq!(events.len(), 3);
        assert!(matches!(events[0].kind, EventKind::RunStarted));
        assert!(matches!(events[2].kind, EventKind::RunCompleted));
    }

    #[tokio::test]
    async fn writer_concurrent_appends_preserve_jsonl_boundaries() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("events.jsonl");
        let writer = EventWriter::new(&path);
        let run_id = RunId("run-concurrent".to_string());

        let mut handles = Vec::new();
        for idx in 0..32 {
            let writer = writer.clone();
            let run_id = run_id.clone();
            handles.push(tokio::spawn(async move {
                let event = Event::new(run_id, EventKind::TaskOutput)
                    .with_payload(serde_json::json!({ "idx": idx }))
                    .unwrap();
                writer.append(&event).await.unwrap();
            }));
        }

        for handle in handles {
            handle.await.unwrap();
        }

        let summary = EventReader::summary(&path).await.unwrap();
        assert_eq!(summary.valid_events, 32);
        assert_eq!(summary.parse_failures, 0);
    }

    #[tokio::test]
    async fn reader_tolerates_partial_trailing_line() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("events.jsonl");

        // Write a valid event followed by a partial line
        let valid = Event::new(RunId("r".to_string()), EventKind::RunStarted);
        let valid_json = serde_json::to_string(&valid).unwrap();
        let mut file = tokio::fs::File::create(&path).await.unwrap();
        use tokio::io::AsyncWriteExt;
        file.write_all(format!("{}\n", valid_json).as_bytes())
            .await
            .unwrap();
        file.write_all(b"{\"partial\": true").await.unwrap(); // incomplete JSON

        let events = EventReader::read_all(&path).await.unwrap();
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0].kind, EventKind::RunStarted));
    }

    #[tokio::test]
    async fn reader_tolerates_malformed_lines() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("events.jsonl");

        let valid = Event::new(RunId("r".to_string()), EventKind::RunStarted);
        let valid_json = serde_json::to_string(&valid).unwrap();
        let mut file = tokio::fs::File::create(&path).await.unwrap();
        use tokio::io::AsyncWriteExt;
        file.write_all(format!("{}\n", valid_json).as_bytes())
            .await
            .unwrap();
        file.write_all(b"not json at all\n").await.unwrap();
        file.write_all(b"{}\n").await.unwrap(); // empty object - will fail because it lacks required fields

        let events = EventReader::read_all(&path).await.unwrap();
        assert_eq!(events.len(), 1);
    }

    #[tokio::test]
    async fn reader_summary() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("events.jsonl");

        let valid = Event::new(RunId("r".to_string()), EventKind::RunStarted);
        let valid_json = serde_json::to_string(&valid).unwrap();
        let mut file = tokio::fs::File::create(&path).await.unwrap();
        use tokio::io::AsyncWriteExt;
        file.write_all(format!("{}\n", valid_json).as_bytes())
            .await
            .unwrap();
        file.write_all(b"bad\n").await.unwrap();
        file.write_all(b"\n").await.unwrap();

        let summary = EventReader::summary(&path).await.unwrap();
        assert_eq!(summary.total_lines, 3);
        assert_eq!(summary.valid_events, 1);
        assert_eq!(summary.parse_failures, 1);
        assert_eq!(summary.empty_lines, 1);
    }

    #[tokio::test]
    async fn reader_filter_by_kind() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("events.jsonl");

        let writer = EventWriter::new(&path);
        let run_id = RunId("run-1".to_string());

        writer
            .append_many(&[
                Event::new(run_id.clone(), EventKind::RunStarted),
                Event::new(run_id.clone(), EventKind::WorkerStarted).with_actor("w1"),
                Event::new(run_id.clone(), EventKind::RunCompleted),
            ])
            .await
            .unwrap();

        let filtered =
            EventReader::read_filtered(&path, &[EventKind::RunStarted, EventKind::RunCompleted])
                .await
                .unwrap();
        assert_eq!(filtered.len(), 2);
    }

    #[tokio::test]
    async fn reader_filters_by_task_and_gate() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("events.jsonl");

        let writer = EventWriter::new(&path);
        let run_id = RunId("run-1".to_string());
        let builder = EventBuilder::new(run_id);

        writer
            .append_many(&[
                builder
                    .task_claimed(
                        TaskId("task-1".to_string()),
                        WorkerId("worker-1".to_string()),
                        60,
                    )
                    .unwrap(),
                builder
                    .task_completed(
                        TaskId("task-1".to_string()),
                        WorkerId("worker-1".to_string()),
                        Some("done"),
                    )
                    .unwrap(),
                builder.gate_passed_by_name("fmt").unwrap(),
                builder.gate_failed_by_name("test").unwrap(),
            ])
            .await
            .unwrap();

        let task_events = EventReader::read_for_task(&path, "task-1").await.unwrap();
        assert_eq!(task_events.len(), 2);
        assert!(task_events
            .iter()
            .all(|e| payload_string(e, "task_id").as_deref() == Some("task-1")));

        let gate_events = EventReader::read_for_gate(&path, "fmt").await.unwrap();
        assert_eq!(gate_events.len(), 1);
        assert_eq!(
            payload_string(&gate_events[0], "gate_id").as_deref(),
            Some("fmt")
        );

        let named_gate_events = EventReader::read_for_gate(&path, "test").await.unwrap();
        assert_eq!(named_gate_events.len(), 1);
        assert!(matches!(named_gate_events[0].kind, EventKind::GateFailed));
    }

    #[test]
    fn event_builder_helpers() {
        let run_id = RunId::generate();
        let builder = EventBuilder::new(run_id.clone());

        let e1 = builder
            .run_started("team", Path::new("/tmp"), "test")
            .unwrap();
        assert!(matches!(e1.kind, EventKind::RunStarted));

        let e2 = builder
            .worker_started(WorkerId("w1".to_string()), "coder")
            .unwrap();
        assert!(matches!(e2.kind, EventKind::WorkerStarted));
        assert_eq!(e2.actor, Some("w1".to_string()));

        let e3 = builder.run_completed();
        assert!(matches!(e3.kind, EventKind::RunCompleted));
    }

    #[test]
    fn run_started_can_include_kimi_metadata() {
        let run_id = RunId::generate();
        let event = EventBuilder::new(run_id)
            .run_started_with_kimi_metadata(
                "team",
                Path::new("/tmp"),
                "test",
                Some("/usr/local/bin/kimi".to_string()),
                Some("kimi version 1.41.0".to_string()),
                Some("1.9".to_string()),
            )
            .unwrap();

        let payload: RunStartedPayload = serde_json::from_value(event.payload.unwrap()).unwrap();
        assert_eq!(payload.kimi_binary.as_deref(), Some("/usr/local/bin/kimi"));
        assert_eq!(
            payload.kimi_cli_version.as_deref(),
            Some("kimi version 1.41.0")
        );
        assert_eq!(payload.wire_protocol_version.as_deref(), Some("1.9"));
    }

    #[test]
    fn command_and_gate_events_can_include_evidence_payload() {
        let run_id = RunId::generate();
        let builder = EventBuilder::new(run_id);

        let started = builder
            .command_started(GateId("fmt".to_string()), "fmt", "cargo fmt --check", 120)
            .unwrap();
        assert!(matches!(started.kind, EventKind::CommandStarted));
        let started_payload = started.payload.unwrap();
        assert_eq!(
            started_payload.get("command_line").and_then(|v| v.as_str()),
            Some("cargo fmt --check")
        );
        assert_eq!(
            started_payload.get("timeout_secs").and_then(|v| v.as_u64()),
            Some(120)
        );

        let finished = builder
            .command_finished(
                GateId("fmt".to_string()),
                "fmt",
                "cargo fmt --check",
                Some(0),
                false,
                Some("ok"),
                Some(""),
                Some("/tmp/gates/fmt.log"),
            )
            .unwrap();
        assert!(matches!(finished.kind, EventKind::CommandFinished));
        let finished_payload = finished.payload.unwrap();
        assert_eq!(
            finished_payload
                .get("command_line")
                .and_then(|v| v.as_str()),
            Some("cargo fmt --check")
        );
        assert_eq!(
            finished_payload.get("exit_code").and_then(|v| v.as_i64()),
            Some(0)
        );
        assert_eq!(
            finished_payload.get("timed_out").and_then(|v| v.as_bool()),
            Some(false)
        );
        assert_eq!(
            finished_payload.get("output_path").and_then(|v| v.as_str()),
            Some("/tmp/gates/fmt.log")
        );

        let gate_passed = builder
            .gate_passed_with_evidence(
                GateId("fmt".to_string()),
                "fmt",
                true,
                Some("cargo fmt --check"),
                Some(0),
                false,
                Some("ok"),
                Some(""),
                Some("/tmp/gates/fmt.log"),
                Some(120),
            )
            .unwrap();
        assert!(matches!(gate_passed.kind, EventKind::GatePassed));
        let gate_payload = gate_passed.payload.unwrap();
        assert_eq!(
            gate_payload.get("stdout_summary").and_then(|v| v.as_str()),
            Some("ok")
        );
        assert_eq!(
            gate_payload.get("timeout_secs").and_then(|v| v.as_u64()),
            Some(120)
        );
    }
}
