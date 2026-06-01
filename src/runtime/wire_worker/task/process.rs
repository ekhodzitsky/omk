use anyhow::{Context, Result};
use std::path::PathBuf;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use crate::runtime::events::{Event, EventKind, JsonlWriter};
use crate::runtime::wire_worker::hook_executor::{
    discover_hook_subscriptions, HookExecutor, HookResult,
};
use crate::runtime::wire_worker::WireWorkerAdapter;
use crate::runtime::worker::{ResultStatus, WorkerResult, WorkerTask};
use crate::wire::{redact_wire_secrets, EventExt, Request, RequestExt};
use crate::wire::{
    ChildProcessTransport, ProcessWireClient, WireClient, WireClientExt, WireMessage,
};

use super::context_guard;
use super::TaskOutcome;

impl WireWorkerAdapter {
    pub(in crate::runtime::wire_worker) async fn process_task(
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
        let mut client = ProcessWireClient::new(
            ChildProcessTransport::spawn(kimi_bin, project_dir, None, None).await?,
        );
        let external_tools: Option<Vec<crate::wire::ExternalTool>> = if let Some(bridge) =
            &self.mcp_bridge
        {
            Some(
                bridge
                    .external_tools()
                    .await
                    .into_iter()
                    .filter_map(|t| match serde_json::from_value(t) {
                        Ok(v) => Some(v),
                        Err(e) => {
                            warn!(error = %e, "Skipping malformed external tool from MCP bridge");
                            None
                        }
                    })
                    .collect(),
            )
        } else {
            self.spec.external_tools.clone().map(|tools| {
                tools
                    .into_iter()
                    .filter_map(|t| match serde_json::from_value(t) {
                        Ok(v) => Some(v),
                        Err(e) => {
                            warn!(error = %e, "Skipping malformed external tool from worker spec");
                            None
                        }
                    })
                    .collect()
            })
        };
        let hooks = discover_hook_subscriptions(project_dir).await;
        let init_params = crate::wire::InitializeParams {
            protocol_version: crate::wire::WIRE_PROTOCOL_VERSION.to_string(),
            client: Some(crate::wire::ClientInfo {
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
                "expected_wire_protocol_version": crate::wire::WIRE_PROTOCOL_VERSION,
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
        context_guard::warn_if_prompt_exceeds_threshold(&prompt, &self.spec.name, &task.id);
        client.start_prompt(prompt.as_str()).await?;
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
                            match ev.params {
                                crate::wire::Event::TurnEnd => break,
                                crate::wire::Event::StepInterrupted => {
                                    success = false;
                                    failure_reason =
                                        Some("wire step interrupted before turn_end".to_string());
                                    break;
                                }
                                crate::wire::Event::HookTriggered { event, target, hook_count } => {
                                    let hook_event = Event::new(self.run_id.clone(), EventKind::HookTriggered)
                                        .with_actor(&self.spec.name)
                                        .with_payload(serde_json::json!({
                                            "task_id": task.id,
                                            "worker_id": self.spec.name,
                                            "event": event,
                                            "target": target,
                                            "hook_count": hook_count,
                                        }))?;
                                    if let Err(e) = self.event_writer.append(&hook_event).await {
                                        warn!(error = %e, "Failed to emit hook_triggered event");
                                    }
                                }
                                crate::wire::Event::HookResolved { event, target, action, reason, duration_ms } => {
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
                                    if let Err(e) = self.event_writer.append(&hook_event).await {
                                        warn!(error = %e, "Failed to emit hook_resolved event");
                                    }
                                }
                                crate::wire::Event::ContentPart(crate::wire::ContentPart::Text(ref part)) => {
                                    summary_parts.push(part.text.clone());
                                }
                                crate::wire::Event::ContentPart(crate::wire::ContentPart::Think(ref part)) => {
                                    summary_parts.push(part.think.clone());
                                }
                                _ => {
                                    match ev.params.normalized_event_type().as_str() {
                                        "turn_end" => break,
                                        "step_interrupted" => {
                                            success = false;
                                            failure_reason =
                                                Some("wire step interrupted before turn_end".to_string());
                                            break;
                                        }
                                        "thinking" | "text" | "content" => {
                                            if let Some(text) =
                                                ev.params.payload().get("text").and_then(|v| v.as_str())
                                            {
                                                summary_parts.push(text.to_string());
                                            } else if let Some(chunk) =
                                                ev.params.payload().get("chunk").and_then(|v| v.as_str())
                                            {
                                                summary_parts.push(chunk.to_string());
                                            }
                                        }
                                        "turn_begin" | "step_begin" | "tool_call" | "tool_call_part"
                                        | "tool_result" | "status_update" | "approval_response" => {}
                                        "hook_triggered" | "hook_resolved" => {
                                            tracing::debug!(event_type = %ev.params.event_type(), "Known hook event failed deserialization");
                                        }
                                        other => warn!(event_type = %other, "Unknown wire event kind"),
                                    }
                                }
                            }
                        }
                        Ok(WireMessage::Request(req)) if req.method != "request" => {
                            warn!(method = %req.method, "Unknown wire request method, skipping");
                        }
                        Ok(WireMessage::Request(req)) => {
                            let request = req.params;
                                context_guard::warn_if_request_exceeds_threshold(&request, &self.spec.name, &task.id);
                                if let crate::wire::Request::HookRequest(ref hook_req) = request {
                                    let hook_result = if let Some(dir) = project_dir {
                                        match HookExecutor::new(dir).run(hook_req).await {
                                            Ok(result) => result,
                                            Err(e) => {
                                                warn!(error = %e, "Hook execution failed");
                                                HookResult {
                                                    action: crate::wire::HookAction::Block,
                                                    reason: format!("hook execution failed: {e}"),
                                                }
                                            }
                                        }
                                    } else {
                                        HookResult::default_allow()
                                    };
                                    let response = hook_result.to_response_value(&hook_req.id);
                                    client.send_response(&req.id, &response).await?;
                                    if let Err(e) = self.record_wire_request(task, &req.id, &request, &response).await {
                                        warn!(error = %e, "Failed to record hook wire request");
                                    }
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
                                if let crate::wire::Request::ToolCallRequest(ref tool_call) = request {
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
                                                    "return_value": crate::wire::ToolReturnValue {
                                                        is_error: false,
                                                        output: crate::wire::ToolOutput::Text(serde_json::to_string(&value).unwrap_or_default()),
                                                        message: String::new(),
                                                        display: vec![],
                                                        extras: None,
                                                    }
                                                }),
                                                Err(e) => serde_json::json!({
                                                    "tool_call_id": tool_call.id,
                                                    "return_value": crate::wire::ToolReturnValue {
                                                        is_error: true,
                                                        output: crate::wire::ToolOutput::Text(String::new()),
                                                        message: e.to_string(),
                                                        display: vec![crate::wire::DisplayBlock::brief("MCP tool call failed")],
                                                        extras: None,
                                                    }
                                                }),
                                            };
                                            self.record_wire_request(task, &req.id, &request, &response).await?;
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
                                match &request {
                                    Request::ApprovalRequest(approval_req) => {
                                        self.handle_approval_request(task, &req.id, approval_req, &mut client, outer_cancel, timeout_cancel).await?;
                                    }
                                    _ => {
                                        let resp = request.default_response();
                                        self.record_wire_request(task, &req.id, &request, &resp)
                                            .await?;
                                        client.send_response(&req.id, resp).await?;
                                        info!(
                                            worker = %self.spec.name,
                                            request_id = %req.id,
                                            request_type = request.kind(),
                                            "Handled wire request"
                                        );
                                    }
                                }
                            }
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
}
