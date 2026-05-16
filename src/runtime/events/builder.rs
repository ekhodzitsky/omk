use anyhow::Result;
use std::path::Path;

use crate::runtime::events::{
    CommandFinishedPayload, CommandStartedPayload, Event, EventKind, FileChangedPayload, GateId,
    GateResultPayload, ProofWrittenPayload, RunId, RunStartedPayload, TaskClaimedPayload,
    TaskCompletedPayload, WorkerHeartbeatPayload, WorkerId, WorkerStartedPayload,
};

/// Convenience builder for common event patterns.
#[derive(Debug)]
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
                timestamp: chrono::Utc::now(),
            })
    }

    pub fn task_claimed(
        &self,
        task_id: crate::runtime::events::TaskId,
        worker_id: WorkerId,
        lease_secs: u64,
    ) -> Result<Event> {
        Event::new(self.run_id.clone(), EventKind::TaskClaimed)
            .with_actor(worker_id.0.clone())
            .with_payload(TaskClaimedPayload {
                task_id,
                worker_id,
                lease_deadline: chrono::Utc::now() + chrono::Duration::seconds(lease_secs as i64),
            })
    }

    pub fn task_completed(
        &self,
        task_id: crate::runtime::events::TaskId,
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
