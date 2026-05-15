use anyhow::{Context, Result};
use std::io::SeekFrom;
use tokio::io::{AsyncBufReadExt, AsyncSeekExt};
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use super::task::TaskOutcome;
use crate::runtime::events::JsonlWriter;
use crate::runtime::wire_worker::WireWorkerAdapter;
use crate::runtime::worker::WorkerTask;

fn task_timeout_secs(task: &WorkerTask) -> u64 {
    task.budget_secs
        .filter(|seconds| *seconds > 0)
        .unwrap_or(crate::runtime::wire_worker::DEFAULT_TASK_TIMEOUT_SECS)
}

impl WireWorkerAdapter {
    pub(super) async fn run_loop(&self) -> Result<()> {
        let inbox = &self.spec.inbox;
        let outbox = &self.spec.outbox;
        let heartbeat = &self.spec.heartbeat;
        let wire_events_path = self
            .spec
            .inbox
            .parent()
            .context("inbox must have a parent directory")?
            .join("wire-events.jsonl");

        // Spawn a single writer actor for wire-events.jsonl. All wire events
        // emitted by process_task across iterations share the same actor, so
        // concurrent task processings (and clones to future helpers) cannot
        // interleave partial JSON line writes.
        let wire_events_writer = JsonlWriter::new(wire_events_path);

        let kimi_bin = std::env::var("MOCK_KIMI")
            .ok()
            .or_else(|| {
                which::which("kimi")
                    .ok()
                    .map(|p| p.to_string_lossy().to_string())
            })
            .unwrap_or_else(|| "kimi".to_string());

        let hb_init = serde_json::json!({
            "status": "ready",
            "name": self.spec.name,
            "ts": chrono::Utc::now().to_rfc3339(),
        });
        tokio::fs::write(heartbeat, hb_init.to_string()).await?;

        info!(worker = %self.spec.name, kimi = %kimi_bin, "Wire worker adapter started");

        let mut last_inbox_offset: u64 = 0;

        loop {
            if self.cancel_token.is_cancelled() {
                info!(worker = %self.spec.name, "Wire worker adapter shutting down due to cancellation");
                let hb_stopped = serde_json::json!({
                    "status": "stopped",
                    "name": self.spec.name,
                    "ts": chrono::Utc::now().to_rfc3339(),
                });
                if let Err(e) = tokio::fs::write(heartbeat, hb_stopped.to_string()).await {
                    warn!(error = %e, "Failed to write final heartbeat");
                }
                return Ok(());
            }

            let hb_alive = serde_json::json!({
                "status": "alive",
                "name": self.spec.name,
                "ts": chrono::Utc::now().to_rfc3339(),
            });
            if let Err(e) = tokio::fs::write(heartbeat, hb_alive.to_string()).await {
                warn!(error = %e, "Failed to write heartbeat");
            }

            if inbox.exists() {
                let file = match tokio::fs::OpenOptions::new().read(true).open(inbox).await {
                    Ok(f) => f,
                    Err(e) => {
                        warn!(error = %e, "Failed to open inbox");
                        tokio::time::sleep(crate::runtime::wire_worker::poll_interval()).await;
                        continue;
                    }
                };
                let mut reader = tokio::io::BufReader::new(file);
                let metadata = match reader.get_ref().metadata().await {
                    Ok(m) => m,
                    Err(e) => {
                        warn!(error = %e, "Failed to get inbox metadata");
                        tokio::time::sleep(crate::runtime::wire_worker::poll_interval()).await;
                        continue;
                    }
                };
                let file_len = metadata.len();

                if file_len < last_inbox_offset {
                    last_inbox_offset = 0;
                }

                if let Err(e) = reader.seek(SeekFrom::Start(last_inbox_offset)).await {
                    warn!(error = %e, "Failed to seek inbox");
                    tokio::time::sleep(crate::runtime::wire_worker::poll_interval()).await;
                    continue;
                }

                let mut line = String::new();
                loop {
                    line.clear();
                    let bytes_read = match reader.read_line(&mut line).await {
                        Ok(n) => n,
                        Err(e) => {
                            warn!(error = %e, "Failed to read inbox line");
                            break;
                        }
                    };
                    if bytes_read == 0 {
                        break;
                    }
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    match serde_json::from_str::<WorkerTask>(trimmed) {
                        Ok(task) => {
                            let timeout_secs = task_timeout_secs(&task);

                            // Per-task cancel token, fired by a sleep task
                            // once the budget elapses. process_task observes
                            // this token (and the outer worker-shutdown token)
                            // in distinct select! arms — the arm that fires
                            // produces the right `TaskOutcome` variant, which
                            // is race-free.
                            let timeout_token = CancellationToken::new();
                            let timer_token = timeout_token.clone();
                            let timer = tokio::spawn(async move {
                                tokio::time::sleep(std::time::Duration::from_secs(timeout_secs))
                                    .await;
                                timer_token.cancel();
                            });

                            let result = self
                                .process_task(
                                    &task,
                                    &kimi_bin,
                                    outbox,
                                    &wire_events_writer,
                                    &self.cancel_token,
                                    &timeout_token,
                                )
                                .await;

                            timer.abort();

                            match result {
                                Err(e) => {
                                    warn!(
                                        error = %e,
                                        worker = %self.spec.name,
                                        task = %task.id,
                                        "Task processing failed"
                                    );
                                }
                                Ok(TaskOutcome::CancelledTimeout) => {
                                    warn!(
                                        worker = %self.spec.name,
                                        task = %task.id,
                                        timeout_secs,
                                        "Task processing timed out"
                                    );
                                    self.record_task_timeout(&task, outbox, timeout_secs)
                                        .await?;
                                }
                                Ok(TaskOutcome::CancelledExternal) => {
                                    // Worker shutdown is in flight; the outer
                                    // loop's cancel check will exit next pass.
                                    // No outbox record — there is no scheduler
                                    // expecting one for a torn-down worker.
                                }
                                Ok(TaskOutcome::Completed) => {}
                            }
                        }
                        Err(e) => {
                            warn!(line = %trimmed, error = %e, "Failed to parse inbox task");
                        }
                    }
                }

                last_inbox_offset = match reader.stream_position().await {
                    Ok(pos) => pos,
                    Err(e) => {
                        warn!(error = %e, "Failed to get stream position");
                        last_inbox_offset
                    }
                };
            }

            tokio::select! {
                biased;
                _ = self.cancel_token.cancelled() => {
                    info!(worker = %self.spec.name, "Wire worker adapter shutting down due to cancellation");
                    let hb_stopped = serde_json::json!({
                        "status": "stopped",
                        "name": self.spec.name,
                        "ts": chrono::Utc::now().to_rfc3339(),
                    });
                    if let Err(e) = tokio::fs::write(heartbeat, hb_stopped.to_string()).await {
                        warn!(error = %e, "Failed to write final heartbeat");
                    }
                    return Ok(());
                }
                _ = tokio::time::sleep(crate::runtime::wire_worker::poll_interval()) => {}
            }
        }
    }
}
