use crate::runtime::events::{
    Event, EventBuilder, EventKind, EventWriter, RunId, TaskId, WorkerId,
};
use crate::runtime::worker::{ResultStatus, WorkerResult, WorkerSpec, WorkerTask};
use crate::wire::client::{WireClient, WireMessage};
use crate::wire::protocol::{Request, RequestParams};
use anyhow::Result;
use std::io::SeekFrom;
use std::path::PathBuf;
use tokio::io::{AsyncBufReadExt, AsyncSeekExt, AsyncWriteExt};
use tracing::{info, warn};

/// Poll interval for the wire worker inbox check loop.
pub const POLL_INTERVAL_SECS: u64 = 5;

/// Adapts a worker spec to the Kimi Wire Protocol.
/// Runs as a background task: polls inbox, spawns `kimi --wire`, processes messages,
/// writes results to outbox, and maintains heartbeat.
pub struct WireWorkerAdapter {
    spec: WorkerSpec,
    run_id: RunId,
    event_writer: EventWriter,
}

impl WireWorkerAdapter {
    pub fn new(spec: WorkerSpec, run_id: RunId, event_writer: EventWriter) -> Self {
        Self {
            spec,
            run_id,
            event_writer,
        }
    }

    /// Spawn the adapter as a background Tokio task.
    pub fn spawn(self) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            if let Err(e) = self.run_loop().await {
                warn!(error = %e, worker = %self.spec.name, "Wire worker adapter failed");
            }
        })
    }

    async fn run_loop(&self) -> Result<()> {
        let inbox = &self.spec.inbox;
        let outbox = &self.spec.outbox;
        let heartbeat = &self.spec.heartbeat;
        let wire_events_path = self.spec.inbox.parent().unwrap().join("wire-events.jsonl");

        // Resolve kimi binary (mock override for tests)
        let kimi_bin = std::env::var("MOCK_KIMI")
            .ok()
            .or_else(|| {
                which::which("kimi")
                    .ok()
                    .map(|p| p.to_string_lossy().to_string())
            })
            .unwrap_or_else(|| "kimi".to_string());

        // Write initial heartbeat
        let hb_init = serde_json::json!({
            "status": "ready",
            "name": self.spec.name,
            "ts": chrono::Utc::now().to_rfc3339(),
        });
        tokio::fs::write(heartbeat, hb_init.to_string()).await?;

        info!(worker = %self.spec.name, kimi = %kimi_bin, "Wire worker adapter started");

        let mut last_inbox_offset: u64 = 0;

        loop {
            // Update heartbeat
            let hb_alive = serde_json::json!({
                "status": "alive",
                "name": self.spec.name,
                "ts": chrono::Utc::now().to_rfc3339(),
            });
            if let Err(e) = tokio::fs::write(heartbeat, hb_alive.to_string()).await {
                warn!(error = %e, "Failed to write heartbeat");
            }

            // Check inbox for new tasks
            if inbox.exists() {
                let file = match tokio::fs::OpenOptions::new().read(true).open(inbox).await {
                    Ok(f) => f,
                    Err(e) => {
                        warn!(error = %e, "Failed to open inbox");
                        tokio::time::sleep(std::time::Duration::from_secs(POLL_INTERVAL_SECS))
                            .await;
                        continue;
                    }
                };
                let mut reader = tokio::io::BufReader::new(file);
                let metadata = match reader.get_ref().metadata().await {
                    Ok(m) => m,
                    Err(e) => {
                        warn!(error = %e, "Failed to get inbox metadata");
                        tokio::time::sleep(std::time::Duration::from_secs(POLL_INTERVAL_SECS))
                            .await;
                        continue;
                    }
                };
                let file_len = metadata.len();

                if file_len < last_inbox_offset {
                    last_inbox_offset = 0;
                }

                if let Err(e) = reader.seek(SeekFrom::Start(last_inbox_offset)).await {
                    warn!(error = %e, "Failed to seek inbox");
                    tokio::time::sleep(std::time::Duration::from_secs(POLL_INTERVAL_SECS)).await;
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
                            match tokio::time::timeout(
                                std::time::Duration::from_secs(300),
                                self.process_task(&task, &kimi_bin, outbox, &wire_events_path),
                            )
                            .await
                            {
                                Ok(Err(e)) => {
                                    warn!(
                                        error = %e,
                                        worker = %self.spec.name,
                                        task = %task.id,
                                        "Task processing failed"
                                    );
                                }
                                Err(_) => {
                                    warn!(
                                        worker = %self.spec.name,
                                        task = %task.id,
                                        "Task processing timed out after 300s"
                                    );
                                }
                                Ok(Ok(())) => {}
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

            tokio::time::sleep(std::time::Duration::from_secs(POLL_INTERVAL_SECS)).await;
        }
    }

    async fn process_task(
        &self,
        task: &WorkerTask,
        kimi_bin: &str,
        outbox: &PathBuf,
        wire_events_path: &PathBuf,
    ) -> Result<()> {
        info!(worker = %self.spec.name, task = %task.id, "Processing task via wire");

        // Emit TaskStarted event
        let started = Event::new(self.run_id.clone(), EventKind::TaskStarted)
            .with_actor(&self.spec.name)
            .with_payload(serde_json::json!({
                "task_id": task.id,
                "worker_id": self.spec.name,
            }))?;
        self.event_writer.append(&started).await?;

        let project_dir = self.spec.project_dir.as_deref();

        // Spawn wire client
        let mut client = WireClient::spawn(kimi_bin, project_dir, None, None)?;

        // Initialize handshake
        let init_params = crate::wire::protocol::InitializeParams {
            protocol_version: crate::wire::protocol::KIMI_WIRE_PROTOCOL_VERSION.to_string(),
            client: Some(crate::wire::protocol::ClientInfo {
                name: "omk-wire-worker".to_string(),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
            }),
            external_tools: None,
            capabilities: None,
            hooks: None,
        };
        let init_result = client.initialize(init_params).await?;
        let init_event = Event::new(self.run_id.clone(), EventKind::TaskOutput)
            .with_actor(&self.spec.name)
            .with_payload(serde_json::json!({
                "type": "wire_initialize",
                "task_id": task.id,
                "worker_id": self.spec.name,
                "kimi_binary": kimi_bin,
                "expected_wire_protocol_version": crate::wire::protocol::KIMI_WIRE_PROTOCOL_VERSION,
                "wire_protocol_version": init_result.protocol_version,
            }))?;
        self.event_writer.append(&init_event).await?;

        // Build prompt
        let prompt = format!(
            "You are a {} agent named {}.\n\nTask: {}\n\nWhen complete, summarize what you did in 1-2 sentences.",
            self.spec.role, self.spec.name, task.task
        );

        // Send prompt
        let _prompt_result = client.prompt(&prompt).await?;

        // Process wire messages
        let mut summary_parts: Vec<String> = Vec::new();
        let mut success = true;
        let start_time = std::time::Instant::now();

        loop {
            match client.read_message().await {
                Ok(WireMessage::Event(ev)) => {
                    // Record raw wire event for audit / replay
                    let raw_json = serde_json::to_string(&ev)?;
                    let mut file = tokio::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(wire_events_path)
                        .await?;
                    file.write_all(raw_json.as_bytes()).await?;
                    file.write_all(b"\n").await?;
                    file.flush().await?;
                    drop(file);

                    // Try typed conversion
                    match ev.params.to_event() {
                        Ok(typed) => match typed {
                            crate::wire::protocol::Event::TurnEnd => break,
                            crate::wire::protocol::Event::StepInterrupted => {
                                success = false;
                                break;
                            }
                            _ => {}
                        },
                        Err(_) => {
                            // Fallback: match by event_type string
                            match ev.params.event_type.as_str() {
                                "turn_end" => break,
                                "step_interrupted" => {
                                    success = false;
                                    break;
                                }
                                "thinking" | "text" | "content" => {
                                    if let Some(text) =
                                        ev.params.payload.get("text").and_then(|v| v.as_str())
                                    {
                                        summary_parts.push(text.to_string());
                                    } else if let Some(chunk) =
                                        ev.params.payload.get("chunk").and_then(|v| v.as_str())
                                    {
                                        summary_parts.push(chunk.to_string());
                                    }
                                }
                                other => {
                                    warn!(event_type = %other, "Unknown wire event kind");
                                }
                            }
                        }
                    }
                }
                Ok(WireMessage::Request(req)) if req.method != "request" => {
                    warn!(method = %req.method, "Unknown wire request method, skipping");
                }
                Ok(WireMessage::Request(req)) => match req.params.to_request() {
                    Ok(request) => {
                        let response = request.default_response();
                        self.record_wire_request(task, &req.id, &req.params, &request, &response)
                            .await?;
                        client.send_response(&req.id, response).await?;
                        info!(
                            worker = %self.spec.name,
                            request_id = %req.id,
                            request_type = request.kind(),
                            "Handled wire request"
                        );
                    }
                    Err(_) => {
                        client
                            .send_error(&req.id, -32601, "Unknown request type")
                            .await?;
                    }
                },
                Ok(WireMessage::SuccessResponse(_)) => {
                    // Responses to our own requests — ignore
                }
                Ok(WireMessage::ErrorResponse(err)) => {
                    warn!(error = ?err.error, "Wire error response");
                    success = false;
                    break;
                }
                Err(e) => {
                    warn!(error = %e, "Wire read error, ending task");
                    success = false;
                    break;
                }
            }
        }

        client.shutdown().await?;

        let summary = summary_parts.join(" ").trim().to_string();
        let elapsed = start_time.elapsed().as_secs();

        let result = WorkerResult {
            task_id: task.id.clone(),
            status: if success {
                ResultStatus::Success
            } else {
                ResultStatus::Failed
            },
            summary: if summary.is_empty() {
                "completed".to_string()
            } else {
                summary
            },
            artifacts: vec![],
            elapsed_secs: elapsed,
        };

        // Write result to outbox
        let outbox_line = format!("{}\n", serde_json::to_string(&result)?);
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(outbox)
            .await?;
        file.write_all(outbox_line.as_bytes()).await?;
        file.flush().await?;

        // Emit completion event
        if success {
            let completed = EventBuilder::new(self.run_id.clone()).task_completed(
                TaskId(task.id.clone()),
                WorkerId(self.spec.name.clone()),
                Some(&result.summary),
            )?;
            self.event_writer.append(&completed).await?;
        } else {
            let failed = Event::new(self.run_id.clone(), EventKind::TaskFailed)
                .with_actor(&self.spec.name)
                .with_payload(serde_json::json!({
                    "task_id": task.id,
                    "worker_id": self.spec.name,
                    "error": result.summary,
                }))?;
            self.event_writer.append(&failed).await?;
        }

        info!(
            worker = %self.spec.name,
            task = %task.id,
            success = success,
            elapsed = elapsed,
            "Task finished"
        );

        Ok(())
    }

    async fn record_wire_request(
        &self,
        task: &WorkerTask,
        request_id: &str,
        params: &RequestParams,
        request: &Request,
        response: &serde_json::Value,
    ) -> Result<()> {
        let event = Event::new(self.run_id.clone(), EventKind::TaskOutput)
            .with_actor(&self.spec.name)
            .with_payload(serde_json::json!({
                "type": "wire_request",
                "task_id": task.id,
                "worker_id": self.spec.name,
                "request_id": request_id,
                "request_type": request.kind(),
                "raw_request_type": params.request_type,
                "response": response,
            }))?;
        self.event_writer.append(&event).await
    }
}
