use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::runtime::events::{EventId, GateId, RunId, TaskId, WorkerId};

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

    pub fn with_message(mut self, message: impl Into<String>) -> anyhow::Result<Self> {
        self.payload = Some(serde_json::json!({ "message": message.into() }));
        Ok(self)
    }

    pub fn with_payload(mut self, payload: impl Serialize) -> anyhow::Result<Self> {
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
    TaskProposed,
    TaskAccepted,
    TaskRejected,
    TaskGraphMutated,
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
    GoalPaused,
    GoalResumed,
    GoalBudgetExhausted,
    GoalBudgetExtended,
    BudgetCheckpoint,
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
pub struct TaskGraphMutationPayload {
    pub action: String,
    pub source: String,
    pub task_id: TaskId,
    pub task_graph_path: PathBuf,
    pub proposal_path: PathBuf,
    pub total_tasks_after: usize,
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
