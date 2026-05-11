use anyhow::Result;
use chrono::{DateTime, Utc};
use tokio::io::{AsyncBufReadExt, AsyncSeekExt};
use tracing::{info, warn};

use crate::runtime::config::{HEARTBEAT_FILE, OUTBOX_FILE, WORKERS_DIR};
use crate::runtime::events::{Event, EventBuilder, EventKind, TaskId, WorkerId};
use crate::runtime::scheduler::runner::SimpleResult;
use crate::runtime::scheduler::runner::{ParsedResult, TeamRunner};
use crate::runtime::worker::{ResultStatus, WorkerResult};

impl TeamRunner {
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
}
