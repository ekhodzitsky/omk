use crate::runtime::events::{EventWriter, RunId};
use crate::runtime::worker::WorkerSpec;
use tokio_util::sync::CancellationToken;

mod loop_impl;
mod task;

/// Poll interval for the wire worker inbox check loop.
pub const POLL_INTERVAL_SECS: u64 = 5;
const WIRE_WORKER_POLL_INTERVAL_MS_ENV: &str = "OMK_WIRE_WORKER_POLL_INTERVAL_MS";
const WIRE_WORKER_POLL_INTERVAL_SECS_ENV: &str = "OMK_WIRE_WORKER_POLL_INTERVAL_SECS";
const DEFAULT_TASK_TIMEOUT_SECS: u64 = 300;
const DEFAULT_ACTIVE_TURN_TIMEOUT_SECS: u64 = 90;
const WIRE_TURN_TIMEOUT_MS_ENV: &str = "OMK_WIRE_TURN_TIMEOUT_MS";
const WIRE_TURN_TIMEOUT_SECS_ENV: &str = "OMK_WIRE_TURN_TIMEOUT_SECS";

/// Adapts a worker spec to the Kimi Wire Protocol.
/// Runs as a background task: polls inbox, spawns `kimi --wire`, processes messages,
/// writes results to outbox, and maintains heartbeat.
pub struct WireWorkerAdapter {
    spec: WorkerSpec,
    run_id: RunId,
    event_writer: EventWriter,
    active_turn_timeout: std::time::Duration,
    cancel_token: CancellationToken,
}

impl WireWorkerAdapter {
    pub fn new(spec: WorkerSpec, run_id: RunId, event_writer: EventWriter) -> Self {
        Self::new_with_cancel(spec, run_id, event_writer, CancellationToken::new())
    }

    pub fn new_with_cancel(
        spec: WorkerSpec,
        run_id: RunId,
        event_writer: EventWriter,
        cancel_token: CancellationToken,
    ) -> Self {
        Self {
            spec,
            run_id,
            event_writer,
            active_turn_timeout: resolve_active_turn_timeout(),
            cancel_token,
        }
    }

    /// Spawn the adapter as a background Tokio task.
    pub fn spawn(self) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            if let Err(e) = self.run_loop().await {
                tracing::warn!(error = %e, worker = %self.spec.name, "Wire worker adapter failed");
            }
        })
    }
}

fn resolve_active_turn_timeout() -> std::time::Duration {
    if let Some(ms) = read_env_u64(WIRE_TURN_TIMEOUT_MS_ENV) {
        return std::time::Duration::from_millis(ms);
    }
    if let Some(secs) = read_env_u64(WIRE_TURN_TIMEOUT_SECS_ENV) {
        return std::time::Duration::from_secs(secs);
    }
    std::time::Duration::from_secs(DEFAULT_ACTIVE_TURN_TIMEOUT_SECS)
}

pub(crate) fn poll_interval() -> std::time::Duration {
    if let Some(ms) = read_env_u64(WIRE_WORKER_POLL_INTERVAL_MS_ENV) {
        return std::time::Duration::from_millis(ms);
    }
    if let Some(secs) = read_env_u64(WIRE_WORKER_POLL_INTERVAL_SECS_ENV) {
        return std::time::Duration::from_secs(secs);
    }
    std::time::Duration::from_secs(POLL_INTERVAL_SECS)
}

fn read_env_u64(key: &str) -> Option<u64> {
    std::env::var(key)
        .ok()
        .and_then(|raw| raw.trim().parse::<u64>().ok())
        .filter(|v| *v > 0)
}
