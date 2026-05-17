use anyhow::{Context, Result};
use std::path::PathBuf;
use tokio::io::AsyncWriteExt;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use crate::runtime::events::{Event, EventBuilder, EventKind, JsonlWriter, TaskId, WorkerId};
use crate::runtime::wire_worker::hook_executor::{discover_hook_subscriptions, HookExecutor};
use crate::runtime::wire_worker::WireWorkerAdapter;
use crate::runtime::worker::{ResultStatus, WorkerResult, WorkerTask};
use crate::wire::client::{ProcessWireClient, WireClient, WireMessage};
use crate::wire::protocol::{redact_wire_secrets, Request, RequestParams};

/// Outcome of [`WireWorkerAdapter::process_task`].
///
/// `Completed` means the task ran to a natural conclusion (success, failure,
/// or wire error) and the result has already been written to the outbox.
///
/// The `Cancelled*` variants both mean cancellation fired before completion —
/// kimi has been killed and no result has been written. They differ in which
/// token fired:
///
/// - [`TaskOutcome::CancelledTimeout`]: the per-task budget elapsed; caller
///   should record a timeout in the outbox so the scheduler sees a failure.
/// - [`TaskOutcome::CancelledExternal`]: the outer worker shutdown token
///   fired; caller should not record anything because the worker itself is
///   tearing down.
///
/// The variant is determined inside the `select!` arm that fires, which
/// eliminates the TOCTOU race that an after-the-fact `is_cancelled()` check
/// has when both tokens fire in quick succession.
pub(super) enum TaskOutcome {
    Completed,
    CancelledTimeout,
    CancelledExternal,
}

impl WireWorkerAdapter {
    pub(super) async fn process_task(
        &self,
        task: &WorkerTask,
        kimi_bin: &str,
        outbox: &PathBuf,
        wire_events_writer: &JsonlWriter,
        outer_cancel: &CancellationToken,
        timeout_cancel: &CancellationToken,
    ) -> Result<TaskOutcome> {
        info!(worker = %self.spec.name, task = %task.id, "Processing task via wire");

        let started = Event::new(self.run_id.clone(), EventKind::TaskStarted)
            .with_actor(&self.spec.name)
            .with_payload(serde_json::json!({
                "task_id": task.id,
                "worker_id": self.spec.name,
            }))?;
        self.event_writer.append(&started).await?;

        let project_dir = self.spec.project_dir.as_deref();
        let mut client = ProcessWireClient::spawn(kimi_bin, project_dir, None, None)?;

        let external_tools = if let Some(bridge) = &self.mcp_bridge {
            Some(bridge.external_tools().await)
        } else {
            self.spec.external_tools.clone()
        };

        let hooks = if let Some(dir) = project_dir {
            discover_hook_subscriptions(Some(dir)).await
        } else {
            Vec::new()
        };

        let init_params = crate::wire::protocol::InitializeParams {
            protocol_version: crate::wire::protocol::KIMI_WIRE_PROTOCOL_VERSION.to_string(),
            client: Some(crate::wire::protocol::ClientInfo {
                name: "omk-wire-worker".to_string(),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
            }),
            external_tools,
            capabilities: None,
            hooks: Some(hooks),
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
                _ = outer_cancel.cancelled() => {
                    info!(
                        worker = %self.spec.name,
                        task = %task.id,
                        "Task processing cancelled (worker shutdown)"
                    );
                    // Reap the kimi child immediately so it does not linger as
                    // a zombie / continue mutating the worktree behind our back.
                    let _ = client.shutdown().await;
                    return Ok(TaskOutcome::CancelledExternal);
                }
                _ = timeout_cancel.cancelled() => {
                    info!(
                        worker = %self.spec.name,
                        task = %task.id,
                        "Task processing cancelled (per-task budget timeout)"
                    );
                    let _ = client.shutdown().await;
                    return Ok(TaskOutcome::CancelledTimeout);
                }
                msg = client.read_message_timeout(self.active_turn_timeout) => {
                    match msg {
                        Ok(WireMessage::Event(ev)) => {
                            let raw_json =
                                serde_json::to_string(&redact_wire_secrets(&serde_json::to_value(&ev)?))?;
                            // Route through the single-writer actor so
                            // concurrent producers across the wire-worker
                            // adapter (and any future helpers cloning the
                            // writer) cannot interleave partial lines.
                            let mut line = raw_json.into_bytes();
                            line.push(b'\n');
                            wire_events_writer
                                .append_line(line)
                                .await
                                .with_context(|| {
                                    format!(
                                        "failed to append wire event for task {} (worker {})",
                                        task.id, self.spec.name
                                    )
                                })?;

                            match ev.params.to_event() {
                                Ok(typed) => match typed {
                                    crate::wire::protocol::Event::TurnEnd => break,
                                    crate::wire::protocol::Event::StepInterrupted => {
                                        success = false;
                                        failure_reason =
                                            Some("wire step interrupted before turn_end".to_string());
                                        break;
                                    }
                                    crate::wire::protocol::Event::HookTriggered { event, target, hook_count } => {
                                        let hook_event = Event::new(self.run_id.clone(), EventKind::HookTriggered)
                                            .with_actor(&self.spec.name)
                                            .with_payload(serde_json::json!({
                                                "task_id": task.id,
                                                "worker_id": self.spec.name,
                                                "event": event,
                                                "target": target,
                                                "hook_count": hook_count,
                                            }))?;
                                        self.event_writer.append(&hook_event).await?;
                                    }
                                    crate::wire::protocol::Event::HookResolved { event, target, action, reason, duration_ms } => {
                                        let hook_event = Event::new(self.run_id.clone(), EventKind::HookResolved)
                                            .with_actor(&self.spec.name)
                                            .with_payload(serde_json::json!({
                                                "task_id": task.id,
                                                "worker_id": self.spec.name,
                                                "event": event,
                                                "target": target,
                                                "action": action,
                                                "reason": reason,
                                                "duration_ms": duration_ms,
                                            }))?;
                                        self.event_writer.append(&hook_event).await?;
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
                                        | "tool_result" | "status_update" | "approval_response"
                                        | "hook_triggered" | "hook_resolved" => {}
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
                                if let crate::wire::protocol::Request::HookRequest(ref hook_req) = request {
                                    let hook_executor = if let Some(dir) = project_dir {
                                        HookExecutor::new(dir)
                                    } else {
                                        HookExecutor::new(".")
                                    };
                                    let hook_result = hook_executor.run(hook_req).await?;
                                    let response = hook_result.to_response_value(&hook_req.id);
                                    self.record_wire_request(task, &req.id, &req.params, &request, &response).await?;
                                    client.send_response(&req.id, &response).await?;
                                    info!(
                                        worker = %self.spec.name,
                                        request_id = %req.id,
                                        request_type = request.kind(),
                                        hook_event = %hook_req.event,
                                        hook_action = ?hook_result.action,
                                        "Handled wire hook request"
                                    );
                                    continue;
                                }

                                if let crate::wire::protocol::Request::ToolCallRequest(ref tool_call) = request {
                                    if let Some(bridge) = &self.mcp_bridge {
                                        if bridge.is_mcp_tool(&tool_call.name).await {
                                            let args = match &tool_call.arguments {
                                                Some(s) => serde_json::from_str(s).unwrap_or(serde_json::Value::Null),
                                                None => serde_json::Value::Null,
                                            };
                                            let result = bridge.handle_tool_call(&tool_call.name, args).await;
                                            let response = match result {
                                                Ok(value) => serde_json::json!({
                                                    "tool_call_id": tool_call.id,
                                                    "return_value": crate::wire::protocol::ToolReturnValue {
                                                        is_error: false,
                                                        output: serde_json::to_string(&value).unwrap_or_default(),
                                                        message: String::new(),
                                                        display: None,
                                                        extras: None,
                                                    }
                                                }),
                                                Err(e) => serde_json::json!({
                                                    "tool_call_id": tool_call.id,
                                                    "return_value": crate::wire::protocol::ToolReturnValue {
                                                        is_error: true,
                                                        output: String::new(),
                                                        message: e.to_string(),
                                                        display: Some(vec![crate::wire::protocol::DisplayBlock::Brief(
                                                            crate::wire::protocol::BriefDisplayBlock {
                                                                text: "MCP tool call failed".to_string(),
                                                            }
                                                        )]),
                                                        extras: None,
                                                    }
                                                }),
                                            };
                                            self.record_wire_request(task, &req.id, &req.params, &request, &response).await?;
                                            client.send_response(&req.id, response).await?;
                                            info!(
                                                worker = %self.spec.name,
                                                request_id = %req.id,
                                                request_type = request.kind(),
                                                tool_name = %tool_call.name,
                                                "Handled MCP tool call via bridge"
                                            );
                                            continue;
                                        }
                                    }
                                }
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

        Ok(TaskOutcome::Completed)
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
