use anyhow::Result;
use std::path::PathBuf;
use tokio::io::AsyncWriteExt;
use tracing::{info, warn};

use crate::runtime::events::{Event, EventBuilder, EventKind, TaskId, WorkerId};
use crate::runtime::wire_worker::WireWorkerAdapter;
use crate::runtime::worker::{ResultStatus, WorkerResult, WorkerTask};
use crate::wire::client::{WireClient, WireMessage};
use crate::wire::protocol::{redact_wire_secrets, Request, RequestParams};

impl WireWorkerAdapter {
    pub(super) async fn process_task(
        &self,
        task: &WorkerTask,
        kimi_bin: &str,
        outbox: &PathBuf,
        wire_events_path: &PathBuf,
    ) -> Result<()> {
        info!(worker = %self.spec.name, task = %task.id, "Processing task via wire");

        let started = Event::new(self.run_id.clone(), EventKind::TaskStarted)
            .with_actor(&self.spec.name)
            .with_payload(serde_json::json!({
                "task_id": task.id,
                "worker_id": self.spec.name,
            }))?;
        self.event_writer.append(&started).await?;

        let project_dir = self.spec.project_dir.as_deref();
        let mut client = WireClient::spawn(kimi_bin, project_dir, None, None)?;

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

        let mut prompt = format!(
            "You are a {} agent named {}.\n\nTask: {}",
            self.spec.role, self.spec.name, task.task
        );
        if !task.acceptance_criteria.is_empty() {
            prompt.push_str("\n\nAcceptance criteria:");
            for item in &task.acceptance_criteria {
                prompt.push_str("\n- ");
                prompt.push_str(item);
            }
        }
        if let Some(context) = task
            .context
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            prompt.push_str("\n\nController context:\n");
            prompt.push_str(context);
        }
        prompt.push_str("\n\nWhen complete, summarize what you did in 1-2 sentences.");
        client.start_prompt(&prompt).await?;

        let mut summary_parts: Vec<String> = Vec::new();
        let mut success = true;
        let mut failure_reason: Option<String> = None;
        let start_time = std::time::Instant::now();

        loop {
            tokio::select! {
                biased;
                _ = self.cancel_token.cancelled() => {
                    info!(worker = %self.spec.name, task = %task.id, "Task processing cancelled");
                    success = false;
                    failure_reason = Some("cancelled by user".to_string());
                    break;
                }
                msg = client.read_message_timeout(self.active_turn_timeout) => {
                    match msg {
                        Ok(WireMessage::Event(ev)) => {
                            let raw_json =
                                serde_json::to_string(&redact_wire_secrets(&serde_json::to_value(&ev)?))?;
                            let mut file = tokio::fs::OpenOptions::new()
                                .create(true)
                                .append(true)
                                .open(wire_events_path)
                                .await?;
                            file.write_all(raw_json.as_bytes()).await?;
                            file.write_all(b"\n").await?;
                            file.flush().await?;
                            drop(file);

                            match ev.params.to_event() {
                                Ok(typed) => match typed {
                                    crate::wire::protocol::Event::TurnEnd => break,
                                    crate::wire::protocol::Event::StepInterrupted => {
                                        success = false;
                                        failure_reason =
                                            Some("wire step interrupted before turn_end".to_string());
                                        break;
                                    }
                                    _ => {}
                                },
                                Err(_) => {
                                    match ev.params.normalized_event_type().as_str() {
                                        "turn_end" => break,
                                        "step_interrupted" => {
                                            success = false;
                                            failure_reason =
                                                Some("wire step interrupted before turn_end".to_string());
                                            break;
                                        }
                                        "thinking" | "text" | "content" | "content_part" => {
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
                                        "turn_begin" | "step_begin" | "tool_call" | "tool_call_part"
                                        | "tool_result" | "status_update" | "approval_response" => {}
                                        other => warn!(event_type = %other, "Unknown wire event kind"),
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
                        Ok(WireMessage::SuccessResponse(_)) => {}
                        Ok(WireMessage::ErrorResponse(err)) => {
                            warn!(error = ?err.error, "Wire error response");
                            success = false;
                            failure_reason = Some(format!(
                                "wire error response: {} (code: {})",
                                err.error.message, err.error.code
                            ));
                            break;
                        }
                        Err(e) => {
                            warn!(error = %e, "Wire read error, ending task");
                            success = false;
                            let reason = e.to_string();
                            if reason.contains("timed out") {
                                let timeout_event =
                                    Event::new(self.run_id.clone(), EventKind::TaskOutput)
                                        .with_actor(&self.spec.name)
                                        .with_payload(serde_json::json!({
                                            "type": "wire_turn_timeout",
                                            "task_id": task.id,
                                            "worker_id": self.spec.name,
                                            "timeout_ms": self.active_turn_timeout.as_millis(),
                                            "error": reason,
                                        }))?;
                                self.event_writer.append(&timeout_event).await?;

                                let stalled = Event::new(
                                    self.run_id.clone(),
                                    EventKind::WorkerStalled,
                                )
                                .with_actor(&self.spec.name)
                                .with_message(format!(
                                    "wire turn timed out after {:?}",
                                    self.active_turn_timeout
                                ))?;
                                self.event_writer.append(&stalled).await?;
                            }
                            failure_reason = Some(reason);
                            break;
                        }
                    }
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
                if success {
                    "completed".to_string()
                } else {
                    failure_reason.unwrap_or_else(|| "wire task failed".to_string())
                }
            } else {
                summary
            },
            artifacts: vec![],
            elapsed_secs: elapsed,
        };

        self.write_worker_result(outbox, &result).await?;

        info!(
            worker = %self.spec.name,
            task = %task.id,
            success = success,
            elapsed = elapsed,
            "Task finished"
        );

        Ok(())
    }

    pub(super) async fn record_task_timeout(
        &self,
        task: &WorkerTask,
        outbox: &PathBuf,
        timeout_secs: u64,
    ) -> Result<()> {
        let result = WorkerResult {
            task_id: task.id.clone(),
            status: ResultStatus::Failed,
            summary: format!("task budget timed out after {timeout_secs}s"),
            artifacts: vec![],
            elapsed_secs: timeout_secs,
        };

        self.write_worker_result(outbox, &result).await?;
        let timeout_event = Event::new(self.run_id.clone(), EventKind::TaskOutput)
            .with_actor(&self.spec.name)
            .with_payload(serde_json::json!({
                "type": "task_budget_timeout",
                "task_id": task.id,
                "worker_id": self.spec.name,
                "timeout_secs": timeout_secs,
            }))?;
        self.event_writer.append(&timeout_event).await?;

        Ok(())
    }

    async fn write_worker_result(&self, outbox: &PathBuf, result: &WorkerResult) -> Result<()> {
        let outbox_line = format!("{}\n", serde_json::to_string(&result)?);
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(outbox)
            .await?;
        file.write_all(outbox_line.as_bytes()).await?;
        file.flush().await?;

        if matches!(result.status, ResultStatus::Success) {
            let completed = EventBuilder::new(self.run_id.clone()).task_completed(
                TaskId(result.task_id.clone()),
                WorkerId(self.spec.name.clone()),
                Some(&result.summary),
            )?;
            self.event_writer.append(&completed).await?;
        } else {
            let failed = Event::new(self.run_id.clone(), EventKind::TaskFailed)
                .with_actor(&self.spec.name)
                .with_payload(serde_json::json!({
                    "task_id": result.task_id.clone(),
                    "worker_id": self.spec.name.clone(),
                    "error": result.summary.clone(),
                }))?;
            self.event_writer.append(&failed).await?;
        }

        Ok(())
    }

    pub(super) async fn record_wire_request(
        &self,
        task: &WorkerTask,
        request_id: &str,
        params: &RequestParams,
        request: &Request,
        response: &serde_json::Value,
    ) -> Result<()> {
        let redacted_request_payload = redact_wire_secrets(&params.payload);
        let redacted_response = redact_wire_secrets(response);
        let event = Event::new(self.run_id.clone(), EventKind::TaskOutput)
            .with_actor(&self.spec.name)
            .with_payload(serde_json::json!({
                "type": "wire_request",
                "task_id": task.id,
                "worker_id": self.spec.name,
                "request_id": request_id,
                "request_type": request.kind(),
                "raw_request_type": params.request_type,
                "request_payload": redacted_request_payload,
                "response": redacted_response,
            }))?;
        self.event_writer.append(&event).await
    }
}
