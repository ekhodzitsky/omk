use anyhow::{Context, Result};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::VecDeque;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout};
use tokio::time::Duration;
use tracing::{debug, info, warn};

use crate::wire::protocol::*;

const LEGACY_NO_HANDSHAKE_PROTOCOL_VERSION: &str = "legacy/no-handshake";

/// A client for communicating with Kimi Code CLI in wire mode.
pub struct WireClient {
    child: Child,
    stdin: ChildStdin,
    stdout_reader: BufReader<ChildStdout>,
    pending_messages: VecDeque<WireMessage>,
    request_id_counter: u64,
    handshake_done: bool,
}

impl WireClient {
    /// Spawn a new kimi process in wire mode.
    pub fn spawn(
        kimi_binary: &str,
        work_dir: Option<&std::path::Path>,
        session: Option<&str>,
        model: Option<&str>,
    ) -> Result<Self> {
        let mut child = None;
        for attempt in 0..3 {
            let mut cmd = tokio::process::Command::new(kimi_binary);
            cmd.arg("--wire");
            if let Some(dir) = work_dir {
                cmd.arg("--work-dir").arg(dir);
            }
            if let Some(s) = session {
                cmd.arg("--session").arg(s);
            }
            if let Some(m) = model {
                cmd.arg("--model").arg(m);
            }
            cmd.stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());

            match cmd.kill_on_drop(true).spawn() {
                Ok(spawned) => {
                    child = Some(spawned);
                    break;
                }
                Err(err) if err.raw_os_error() == Some(26) && attempt < 2 => {
                    std::thread::sleep(Duration::from_millis(25));
                }
                Err(err) => return Err(err).context("Failed to spawn kimi --wire"),
            }
        }

        let mut child = child.context("Failed to spawn kimi --wire")?;
        let stdin = child.stdin.take().context("No stdin")?;
        let stdout = child.stdout.take().context("No stdout")?;
        let stdout_reader = BufReader::new(stdout);

        info!("Wire client spawned");

        Ok(Self {
            child,
            stdin,
            stdout_reader,
            pending_messages: VecDeque::new(),
            request_id_counter: 0,
            handshake_done: false,
        })
    }

    /// Send initialize handshake.
    pub async fn initialize(&mut self, params: InitializeParams) -> Result<InitializeResult> {
        let id = self.next_id();
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "initialize".to_string(),
            id: id.clone(),
            params,
        };
        self.send_request(&req).await?;

        let mut line = String::new();
        self.stdout_reader
            .read_line(&mut line)
            .await
            .context("Failed to read initialize response")?;
        if line.is_empty() {
            anyhow::bail!("kimi stdout closed while waiting for initialize response");
        }

        // Check for method-not-found error (-32601)
        if let Ok(error_resp) = serde_json::from_str::<JsonRpcErrorResponse>(&line) {
            if error_resp.error.code == -32601 {
                warn!(
                    code = error_resp.error.code,
                    "Server does not support initialize, falling back to legacy no-handshake mode"
                );
                self.handshake_done = true;
                return Ok(InitializeResult {
                    protocol_version: LEGACY_NO_HANDSHAKE_PROTOCOL_VERSION.to_string(),
                    server: None,
                    slash_commands: None,
                    external_tools: None,
                    capabilities: None,
                    hooks: None,
                });
            }
            anyhow::bail!(
                "Initialize failed: {} (code: {})",
                error_resp.error.message,
                error_resp.error.code
            );
        }

        let resp: JsonRpcSuccessResponse<InitializeResult> =
            serde_json::from_str(&line).context("Failed to parse initialize response")?;
        self.handshake_done = true;
        Ok(resp.result)
    }

    /// Send a prompt and start a turn.
    pub async fn prompt(&mut self, user_input: &str) -> Result<PromptResult> {
        let id = self.start_prompt(user_input).await?;
        self.read_response::<PromptResult>(&id).await
    }

    /// Send a prompt without waiting for the final prompt response.
    ///
    /// Real Kimi may stream `event` / `request` messages before it sends the
    /// JSON-RPC response for `prompt`. Runtime callers that need to handle those
    /// messages must use this method and then drive `read_message`.
    pub async fn start_prompt(&mut self, user_input: &str) -> Result<String> {
        let id = self.next_id();
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "prompt".to_string(),
            id: id.clone(),
            params: PromptParams {
                user_input: UserInput::Text(user_input.to_string()),
            },
        };
        self.send_request(&req).await?;
        Ok(id)
    }

    /// Replay events and requests from the current session.
    pub async fn replay(&mut self) -> Result<ReplayResult> {
        let id = self.next_id();
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "replay".to_string(),
            id: id.clone(),
            params: ReplayParams::default(),
        };
        self.send_request(&req).await?;
        self.read_response::<ReplayResult>(&id).await
    }

    /// Steer the current turn with additional user input.
    pub async fn steer(&mut self, user_input: &str) -> Result<SteerResult> {
        let id = self.next_id();
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "steer".to_string(),
            id: id.clone(),
            params: SteerParams {
                user_input: UserInput::Text(user_input.to_string()),
            },
        };
        self.send_request(&req).await?;
        self.read_response::<SteerResult>(&id).await
    }

    /// Enable or disable plan mode for the current wire session.
    pub async fn set_plan_mode(&mut self, enabled: bool) -> Result<SetPlanModeResult> {
        let id = self.next_id();
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "set_plan_mode".to_string(),
            id: id.clone(),
            params: SetPlanModeParams { enabled },
        };
        self.send_request(&req).await?;
        self.read_response::<SetPlanModeResult>(&id).await
    }

    /// Cancel current turn.
    pub async fn cancel(&mut self) -> Result<()> {
        let id = self.next_id();
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "cancel".to_string(),
            id: id.clone(),
            params: CancelParams::default(),
        };
        self.send_request(&req).await?;
        let _: CancelResult = self.read_response(&id).await?;
        Ok(())
    }

    /// Read the next message from stdout (event, request, or response).
    pub async fn read_message(&mut self) -> Result<WireMessage> {
        if let Some(msg) = self.pending_messages.pop_front() {
            return Ok(msg);
        }

        self.read_message_from_stdout().await
    }

    async fn read_message_from_stdout(&mut self) -> Result<WireMessage> {
        let mut line = String::new();
        self.stdout_reader
            .read_line(&mut line)
            .await
            .context("Failed to read from kimi stdout")?;
        if line.is_empty() {
            anyhow::bail!("kimi stdout closed");
        }
        debug!(line = %line.trim(), "Received wire message");
        let msg: WireMessage =
            serde_json::from_str(&line).context("Failed to parse wire message")?;
        Ok(msg)
    }

    /// Read the next message from stdout with a timeout.
    pub async fn read_message_timeout(&mut self, timeout: Duration) -> Result<WireMessage> {
        match tokio::time::timeout(timeout, self.read_message()).await {
            Ok(msg) => msg,
            Err(_) => anyhow::bail!("Wire message read timed out after {:?}", timeout),
        }
    }

    /// Send a response to an agent request.
    pub async fn send_response<ResultType: Serialize>(
        &mut self,
        id: &str,
        result: ResultType,
    ) -> Result<()> {
        let resp = JsonRpcSuccessResponse {
            jsonrpc: "2.0".to_string(),
            id: id.to_string(),
            result,
        };
        let line = format!("{}\n", serde_json::to_string(&resp)?);
        self.stdin.write_all(line.as_bytes()).await?;
        self.stdin.flush().await?;
        Ok(())
    }

    /// Send an error response.
    pub async fn send_error(&mut self, id: &str, code: i32, message: &str) -> Result<()> {
        let resp = JsonRpcErrorResponse {
            jsonrpc: "2.0".to_string(),
            id: id.to_string(),
            error: JsonRpcError {
                code,
                message: message.to_string(),
                data: None,
            },
        };
        let line = format!("{}\n", serde_json::to_string(&resp)?);
        self.stdin.write_all(line.as_bytes()).await?;
        self.stdin.flush().await?;
        Ok(())
    }

    /// Gracefully shutdown the child process.
    pub async fn shutdown(mut self) -> Result<()> {
        let _ = self.child.kill().await;
        Ok(())
    }

    /// Returns true if the initialize handshake has completed (or was skipped via fallback).
    pub fn is_handshake_done(&self) -> bool {
        self.handshake_done
    }

    fn next_id(&mut self) -> String {
        self.request_id_counter += 1;
        format!("req-{}", self.request_id_counter)
    }

    async fn send_request<Params: Serialize>(
        &mut self,
        req: &JsonRpcRequest<Params>,
    ) -> Result<()> {
        let line = format!("{}\n", serde_json::to_string(req)?);
        self.stdin.write_all(line.as_bytes()).await?;
        self.stdin.flush().await?;
        Ok(())
    }

    async fn read_response<ResultType: DeserializeOwned>(
        &mut self,
        expected_id: &str,
    ) -> Result<ResultType> {
        loop {
            if let Some(idx) = self
                .pending_messages
                .iter()
                .position(|msg| wire_message_id(msg) == Some(expected_id))
            {
                let msg = self
                    .pending_messages
                    .remove(idx)
                    .expect("pending response index should be valid");
                return decode_response(msg, expected_id);
            }

            match self
                .read_message_from_stdout()
                .await
                .context("Failed to read response")?
            {
                WireMessage::SuccessResponse(resp) if resp.id == expected_id => {
                    return serde_json::from_value(resp.result)
                        .context("Failed to decode response result");
                }
                WireMessage::ErrorResponse(resp) if resp.id == expected_id => {
                    return bail_wire_error(resp);
                }
                other => {
                    debug!(message = ?other, "Buffering pre-response wire message");
                    self.pending_messages.push_back(other);
                }
            }
        }
    }
}

fn wire_message_id(msg: &WireMessage) -> Option<&str> {
    match msg {
        WireMessage::SuccessResponse(resp) => Some(resp.id.as_str()),
        WireMessage::ErrorResponse(resp) => Some(resp.id.as_str()),
        WireMessage::Request(req) => Some(req.id.as_str()),
        WireMessage::Event(_) => None,
    }
}

fn decode_response<ResultType: DeserializeOwned>(
    msg: WireMessage,
    expected_id: &str,
) -> Result<ResultType> {
    match msg {
        WireMessage::SuccessResponse(resp) if resp.id == expected_id => {
            serde_json::from_value(resp.result).context("Failed to decode response result")
        }
        WireMessage::ErrorResponse(resp) if resp.id == expected_id => bail_wire_error(resp),
        other => anyhow::bail!(
            "Buffered wire message did not match expected response id {expected_id}: {other:?}"
        ),
    }
}

fn bail_wire_error<ResultType>(resp: JsonRpcErrorResponse) -> Result<ResultType> {
    anyhow::bail!(
        "Wire request failed: {} (code: {})",
        resp.error.message,
        resp.error.code
    )
}

/// A union type for all incoming wire messages.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum WireMessage {
    Request(JsonRpcRequest<RequestParams>),
    Event(JsonRpcNotification<EventParams>),
    SuccessResponse(JsonRpcSuccessResponse<Value>),
    ErrorResponse(JsonRpcErrorResponse),
}

/// A response to be sent back to the agent.
pub struct WireResponse {
    pub id: String,
    pub result: serde_json::Value,
}

/// Process wire messages in a loop, handling events and requests.
pub async fn process_messages<F, Fut>(client: &mut WireClient, mut handler: F) -> Result<()>
where
    F: FnMut(WireMessage) -> Fut,
    Fut: std::future::Future<Output = Result<Option<WireResponse>>>,
{
    loop {
        match client.read_message().await {
            Ok(msg) => {
                match &msg {
                    WireMessage::Request(req) if req.method != "request" => {
                        warn!(method = %req.method, "Unknown wire request method, skipping");
                        continue;
                    }
                    WireMessage::Request(req) if req.params.to_request().is_err() => {
                        warn!(
                            request_id = %req.id,
                            request_type = %req.params.request_type,
                            "Unknown wire request type, replying with error"
                        );
                        client
                            .send_error(&req.id, -32601, "Unknown request type")
                            .await?;
                        continue;
                    }
                    WireMessage::Event(ev) if ev.params.to_event().is_err() => {
                        warn!(event_type = %ev.params.event_type, "Unknown wire event kind");
                        continue;
                    }
                    _ => {}
                }
                if let Some(response) = handler(msg).await? {
                    client.send_response(&response.id, response.result).await?;
                }
            }
            Err(e) => {
                warn!(error = %e, "Wire message error, exiting loop");
                break;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wire_message_parsing_event() {
        let json = r#"{"jsonrpc":"2.0","method":"event","params":{"type":"thinking","payload":{"chunk":"hello"}}}"#;
        let msg: WireMessage = serde_json::from_str(json).unwrap();
        match msg {
            WireMessage::Event(notif) => {
                assert_eq!(notif.method, "event");
                assert_eq!(notif.params.event_type, "thinking");
                assert_eq!(notif.params.payload["chunk"], "hello");
            }
            other => panic!("Expected event, got {:?}", other),
        }
    }

    #[test]
    fn test_wire_message_parsing_request() {
        let json = r#"{"jsonrpc":"2.0","method":"tool_call","id":"req-1","params":{"type":"read_file","payload":{"path":"/tmp/test"}}}"#;
        let msg: WireMessage = serde_json::from_str(json).unwrap();
        match msg {
            WireMessage::Request(req) => {
                assert_eq!(req.method, "tool_call");
                assert_eq!(req.id, "req-1");
                assert_eq!(req.params.request_type, "read_file");
            }
            other => panic!("Expected request, got {:?}", other),
        }
    }

    #[test]
    fn test_wire_message_parsing_success_response() {
        let json = r#"{"jsonrpc":"2.0","id":"req-1","result":{"status":"ok"}}"#;
        let msg: WireMessage = serde_json::from_str(json).unwrap();
        match msg {
            WireMessage::SuccessResponse(resp) => {
                assert_eq!(resp.id, "req-1");
                assert_eq!(resp.result["status"], "ok");
            }
            other => panic!("Expected success response, got {:?}", other),
        }
    }

    #[test]
    fn test_wire_message_parsing_error_response() {
        let json =
            r#"{"jsonrpc":"2.0","id":"req-1","error":{"code":-32600,"message":"Invalid Request"}}"#;
        let msg: WireMessage = serde_json::from_str(json).unwrap();
        match msg {
            WireMessage::ErrorResponse(resp) => {
                assert_eq!(resp.id, "req-1");
                assert_eq!(resp.error.code, -32600);
                assert_eq!(resp.error.message, "Invalid Request");
            }
            other => panic!("Expected error response, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_wire_client_spawn() {
        let tmp = tempfile::tempdir().unwrap();
        let script = tmp.path().join("mock-true");
        let script_content = r#"#!/bin/bash
exit 0
"#;
        tokio::fs::write(&script, script_content).await.unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = tokio::fs::metadata(&script).await.unwrap().permissions();
            perms.set_mode(0o755);
            tokio::fs::set_permissions(&script, perms).await.unwrap();
        }

        let client = WireClient::spawn(script.to_str().unwrap(), None, None, None);
        assert!(client.is_ok());
        let client = client.unwrap();
        client.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_roundtrip_send_request_read_response() {
        let tmp = tempfile::tempdir().unwrap();
        let script = tmp.path().join("mock-wire");
        let script_content = r#"#!/bin/bash
read -r line
echo '{"jsonrpc":"2.0","id":"req-1","result":{"status":"ok","steps":[{"n":1}]}}'
"#;
        tokio::fs::write(&script, script_content).await.unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = tokio::fs::metadata(&script).await.unwrap().permissions();
            perms.set_mode(0o755);
            tokio::fs::set_permissions(&script, perms).await.unwrap();
        }

        let mut client = WireClient::spawn(script.to_str().unwrap(), None, None, None).unwrap();

        let result = client.prompt("hello").await.unwrap();
        assert_eq!(result.status, "ok");
        match result.steps.unwrap() {
            PromptSteps::LegacyTrace(steps) => {
                assert_eq!(steps.len(), 1);
                assert_eq!(steps[0]["n"], 1);
            }
            other => panic!("expected legacy prompt trace, got {:?}", other),
        }

        client.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_prompt_buffers_events_that_arrive_before_response() {
        let tmp = tempfile::tempdir().unwrap();
        let script = tmp.path().join("mock-wire-event-first");
        let script_content = r#"#!/bin/bash
read -r line
echo '{"jsonrpc":"2.0","method":"event","params":{"type":"turn_begin","payload":{"user_input":"hello"}}}'
echo '{"jsonrpc":"2.0","id":"req-1","result":{"status":"ok"}}'
"#;
        tokio::fs::write(&script, script_content).await.unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = tokio::fs::metadata(&script).await.unwrap().permissions();
            perms.set_mode(0o755);
            tokio::fs::set_permissions(&script, perms).await.unwrap();
        }

        let mut client = WireClient::spawn(script.to_str().unwrap(), None, None, None).unwrap();

        let result = client.prompt("hello").await.unwrap();
        assert_eq!(result.status, "ok");

        let buffered = client.read_message().await.unwrap();
        match buffered {
            WireMessage::Event(ev) => assert_eq!(ev.params.event_type, "turn_begin"),
            other => panic!("expected buffered event, got {:?}", other),
        }

        client.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_prompt_waits_for_matching_response_id() {
        let tmp = tempfile::tempdir().unwrap();
        let script = tmp.path().join("mock-wire-interleaved-response");
        let script_content = r#"#!/bin/bash
read -r line
echo '{"jsonrpc":"2.0","id":"req-999","result":{"status":"wrong"}}'
echo '{"jsonrpc":"2.0","method":"event","params":{"type":"turn_begin","payload":{"user_input":"hello"}}}'
echo '{"jsonrpc":"2.0","id":"req-1","result":{"status":"ok"}}'
"#;
        tokio::fs::write(&script, script_content).await.unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = tokio::fs::metadata(&script).await.unwrap().permissions();
            perms.set_mode(0o755);
            tokio::fs::set_permissions(&script, perms).await.unwrap();
        }

        let mut client = WireClient::spawn(script.to_str().unwrap(), None, None, None).unwrap();

        let result = client.prompt("hello").await.unwrap();
        assert_eq!(result.status, "ok");

        let buffered = client.read_message().await.unwrap();
        match buffered {
            WireMessage::SuccessResponse(resp) => assert_eq!(resp.id, "req-999"),
            other => panic!("expected buffered non-matching response, got {:?}", other),
        }

        client.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_start_prompt_allows_streaming_before_response() {
        let tmp = tempfile::tempdir().unwrap();
        let script = tmp.path().join("mock-wire-stream-before-response");
        let script_content = r#"#!/bin/bash
read -r line
echo '{"jsonrpc":"2.0","method":"event","params":{"type":"turn_begin","payload":{"user_input":"hello"}}}'
sleep 1
"#;
        tokio::fs::write(&script, script_content).await.unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = tokio::fs::metadata(&script).await.unwrap().permissions();
            perms.set_mode(0o755);
            tokio::fs::set_permissions(&script, perms).await.unwrap();
        }

        let mut client = WireClient::spawn(script.to_str().unwrap(), None, None, None).unwrap();

        let id = client.start_prompt("hello").await.unwrap();
        assert_eq!(id, "req-1");

        let msg = client.read_message().await.unwrap();
        match msg {
            WireMessage::Event(ev) => assert_eq!(ev.params.event_type, "turn_begin"),
            other => panic!("expected streaming event, got {:?}", other),
        }

        client.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_send_response_and_error() {
        let tmp = tempfile::tempdir().unwrap();
        let script = tmp.path().join("mock-wire-responder");
        let script_content = r#"#!/bin/bash
read -r line
read -r line
"#;
        tokio::fs::write(&script, script_content).await.unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = tokio::fs::metadata(&script).await.unwrap().permissions();
            perms.set_mode(0o755);
            tokio::fs::set_permissions(&script, perms).await.unwrap();
        }

        let mut client = WireClient::spawn(script.to_str().unwrap(), None, None, None).unwrap();

        client
            .send_response("req-42", serde_json::json!({"ok": true}))
            .await
            .unwrap();

        client
            .send_error("req-43", -32600, "Invalid Request")
            .await
            .unwrap();

        client.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_process_messages_loop() {
        let tmp = tempfile::tempdir().unwrap();
        let script = tmp.path().join("mock-wire-events");
        let script_content = r#"#!/bin/bash
echo '{"jsonrpc":"2.0","method":"event","params":{"type":"turn_begin","payload":{"user_input":"hello"}}}'
"#;
        tokio::fs::write(&script, script_content).await.unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = tokio::fs::metadata(&script).await.unwrap().permissions();
            perms.set_mode(0o755);
            tokio::fs::set_permissions(&script, perms).await.unwrap();
        }

        let mut client = WireClient::spawn(script.to_str().unwrap(), None, None, None).unwrap();

        let seen_event = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let seen_clone = seen_event.clone();
        process_messages(&mut client, move |msg| {
            let seen = seen_clone.clone();
            async move {
                if let WireMessage::Event(ev) = msg {
                    if ev.params.event_type == "turn_begin" {
                        seen.store(true, std::sync::atomic::Ordering::SeqCst);
                    }
                }
                Ok(None)
            }
        })
        .await
        .unwrap();

        assert!(seen_event.load(std::sync::atomic::Ordering::SeqCst));
        client.shutdown().await.unwrap();
    }
}
