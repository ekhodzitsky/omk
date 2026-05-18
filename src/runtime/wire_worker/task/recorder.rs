use anyhow::Result;
use std::path::PathBuf;
use tokio::io::AsyncWriteExt;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use crate::runtime::events::{Event, EventBuilder, EventKind, TaskId, WorkerId};
use crate::runtime::wire_worker::{ApprovalDecision, WireWorkerAdapter};
use crate::runtime::worker::{ResultStatus, WorkerResult, WorkerTask};
use crate::wire::client::{ProcessWireClient, WireClient};
use crate::wire::protocol::{redact_wire_secrets, ApprovalRequest, Request, RequestParams};

impl WireWorkerAdapter {
    pub(in crate::runtime::wire_worker) async fn record_task_timeout(
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

    pub(super) async fn write_worker_result(&self, outbox: &PathBuf, result: &WorkerResult) -> Result<()> {
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

    pub(in crate::runtime::wire_worker) async fn record_wire_request(
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

    pub(super) async fn handle_approval_request(
        &self,
        task: &WorkerTask,
        request_id: &str,
        approval_req: &ApprovalRequest,
        client: &mut ProcessWireClient,
        outer_cancel: &CancellationToken,
        timeout_cancel: &CancellationToken,
    ) -> Result<()> {
        let requested = match Event::new(self.run_id.clone(), EventKind::ApprovalRequested)
            .with_actor(&self.spec.name)
            .with_payload(serde_json::json!({
                "task_id": task.id,
                "worker_id": self.spec.name,
                "request_id": request_id,
                "approval_request_id": approval_req.id,
                "tool_call_id": approval_req.tool_call_id,
                "sender": approval_req.sender,
                "action": approval_req.action,
                "description": approval_req.description,
                "source_kind": approval_req.source_kind,
                "agent_id": approval_req.agent_id,
                "subagent_type": approval_req.subagent_type,
            })) {
            Ok(ev) => ev,
            Err(e) => {
                warn!(error = %e, "Failed to build approval_requested event; sending rejection");
                let fallback = serde_json::json!({
                    "request_id": approval_req.id,
                    "response": crate::wire::protocol::ApprovalResponseType::Reject,
                    "feedback": "OMK internal error building approval event.",
                });
                client.send_response(request_id, fallback).await?;
                return Ok(());
            }
        };
        if let Err(e) = self.event_writer.append(&requested).await {
            warn!(error = %e, "Failed to emit approval_requested event; continuing");
        }

        let decision = tokio::select! {
            biased;
            _ = outer_cancel.cancelled() => {
                info!(worker = %self.spec.name, "Approval cancelled by worker shutdown");
                ApprovalDecision::Reject
            }
            _ = timeout_cancel.cancelled() => {
                info!(worker = %self.spec.name, "Approval cancelled by task budget timeout");
                ApprovalDecision::Reject
            }
            d = self.approval_proxy.decide(approval_req) => d,
        };

        let response_type = decision.to_response_type();
        let feedback = match &decision {
            ApprovalDecision::Approve => "OMK approved this request.",
            ApprovalDecision::ApproveForSession => {
                "OMK approved this request for the session."
            }
            ApprovalDecision::Reject => "OMK rejected this request.",
        };

        let response = serde_json::json!({
            "request_id": approval_req.id,
            "response": response_type,
            "feedback": feedback,
        });

        if let Ok(decided) = Event::new(self.run_id.clone(), EventKind::ApprovalDecided)
            .with_actor(&self.spec.name)
            .with_payload(serde_json::json!({
                "task_id": task.id,
                "worker_id": self.spec.name,
                "request_id": request_id,
                "approval_request_id": approval_req.id,
                "decision": match response_type {
                    crate::wire::protocol::ApprovalResponseType::Approve => "approve",
                    crate::wire::protocol::ApprovalResponseType::ApproveForSession => "approve_for_session",
                    crate::wire::protocol::ApprovalResponseType::Reject => "reject",
                },
                "feedback": feedback,
            }))
        {
            if let Err(e) = self.event_writer.append(&decided).await {
                warn!(error = %e, "Failed to emit approval_decided event; continuing");
            }
        }

        let request_params = match serde_json::to_value(approval_req) {
            Ok(payload) => RequestParams {
                request_type: "ApprovalRequest".to_string(),
                payload,
            },
            Err(e) => {
                warn!(error = %e, "Failed to serialize approval request for logging; skipping record_wire_request");
                RequestParams {
                    request_type: "ApprovalRequest".to_string(),
                    payload: serde_json::json!({"error": "serialization failed"}),
                }
            }
        };

        if let Err(e) = self
            .record_wire_request(
                task,
                request_id,
                &request_params,
                &Request::ApprovalRequest(approval_req.clone()),
                &response,
            )
            .await
        {
            warn!(error = %e, "Failed to record wire request; continuing");
        }

        client.send_response(request_id, response).await?;

        info!(
            worker = %self.spec.name,
            request_id = %request_id,
            approval_request_id = %approval_req.id,
            action = %approval_req.action,
            decision = %match decision {
                ApprovalDecision::Approve => "approve",
                ApprovalDecision::ApproveForSession => "approve_for_session",
                ApprovalDecision::Reject => "reject",
            },
            "Handled approval request"
        );

        Ok(())
    }
}
