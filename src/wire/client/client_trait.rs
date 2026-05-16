use std::collections::VecDeque;
use std::time::Duration;

use anyhow::{Context, Result};
use serde::{de::DeserializeOwned, Serialize};
use tokio::sync::Mutex;

use super::WireMessage;
use crate::wire::protocol::{
    CancelParams, CancelResult, InitializeParams, InitializeResult, JsonRpcErrorResponse,
    JsonRpcRequest, PromptParams, PromptResult, ReplayParams, ReplayResult, SetPlanModeParams,
    SetPlanModeResult, SteerParams, SteerResult, UserInput,
};

/// Trait for a Kimi Wire Protocol client.
///
/// Implementations may communicate over a child process (see
/// [`ProcessWireClient`](super::ProcessWireClient)) or through an in-memory
/// channel for testing.
#[allow(async_fn_in_trait)]
pub trait WireClient {
    /// Generate the next request id.
    fn next_id(&mut self) -> String;

    /// Send a JSON-RPC request.
    async fn send_request<Params: Serialize>(&mut self, req: &JsonRpcRequest<Params>)
        -> Result<()>;

    /// Read the next incoming message.
    async fn read_message(&mut self) -> Result<WireMessage>;

    /// Read the next incoming message with a timeout.
    async fn read_message_timeout(&mut self, timeout: Duration) -> Result<WireMessage>;

    /// Wait for a response matching `expected_id`, buffering out-of-order
    /// messages internally.
    async fn read_response<T: DeserializeOwned>(&mut self, expected_id: &str) -> Result<T>;

    /// Send a JSON-RPC success response.
    async fn send_response<T: Serialize>(&mut self, id: &str, result: T) -> Result<()>;

    /// Send a JSON-RPC error response.
    async fn send_error(&mut self, id: &str, code: i32, message: &str) -> Result<()>;

    /// Perform the initialize handshake.
    async fn initialize(&mut self, params: InitializeParams) -> Result<InitializeResult>;

    /// Returns true if the initialize handshake has completed.
    fn is_handshake_done(&self) -> bool;

    /// Gracefully shut down the client.
    async fn shutdown(self) -> Result<()>;

    /// Send a prompt and wait for the result.
    async fn prompt(&mut self, user_input: &str) -> Result<PromptResult> {
        let id = self.start_prompt(user_input).await?;
        self.read_response(&id).await
    }

    /// Send a prompt without waiting for the result.
    async fn start_prompt(&mut self, user_input: &str) -> Result<String> {
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
    async fn replay(&mut self) -> Result<ReplayResult> {
        let id = self.next_id();
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "replay".to_string(),
            id: id.clone(),
            params: ReplayParams::default(),
        };
        self.send_request(&req).await?;
        self.read_response(&id).await
    }

    /// Steer the current turn with additional user input.
    async fn steer(&mut self, user_input: &str) -> Result<SteerResult> {
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
        self.read_response(&id).await
    }

    /// Enable or disable plan mode.
    async fn set_plan_mode(&mut self, enabled: bool) -> Result<SetPlanModeResult> {
        let id = self.next_id();
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "set_plan_mode".to_string(),
            id: id.clone(),
            params: SetPlanModeParams { enabled },
        };
        self.send_request(&req).await?;
        self.read_response(&id).await
    }

    /// Cancel the current turn.
    async fn cancel(&mut self) -> Result<()> {
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
}

/// In-memory wire client for unit tests.
///
/// Holds an internal queue of [`WireMessage`]s that `read_message` drains.
/// Tests inject messages via [`InMemoryWireClient::inject`].
#[derive(Debug)]
pub struct InMemoryWireClient {
    incoming: Mutex<VecDeque<WireMessage>>,
    pending: Mutex<VecDeque<WireMessage>>,
    outgoing: Mutex<Vec<serde_json::Value>>,
    handshake_done: bool,
    request_counter: u64,
}

impl Default for InMemoryWireClient {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryWireClient {
    pub fn new() -> Self {
        Self {
            incoming: Mutex::new(VecDeque::new()),
            pending: Mutex::new(VecDeque::new()),
            outgoing: Mutex::new(Vec::new()),
            handshake_done: false,
            request_counter: 0,
        }
    }

    /// Inject an incoming message for the client to read.
    pub async fn inject(&self, msg: WireMessage) {
        self.incoming.lock().await.push_back(msg);
    }

    /// Access all messages sent by the client.
    pub async fn outgoing(&self) -> Vec<serde_json::Value> {
        self.outgoing.lock().await.clone()
    }
}

#[allow(async_fn_in_trait)]
impl WireClient for InMemoryWireClient {
    fn next_id(&mut self) -> String {
        self.request_counter += 1;
        format!("req-{}", self.request_counter)
    }

    async fn send_request<Params: Serialize>(
        &mut self,
        req: &JsonRpcRequest<Params>,
    ) -> Result<()> {
        let value = serde_json::to_value(req)?;
        self.outgoing.lock().await.push(value);
        Ok(())
    }

    async fn read_message(&mut self) -> Result<WireMessage> {
        if let Some(msg) = self.pending.lock().await.pop_front() {
            return Ok(msg);
        }
        match self.incoming.lock().await.pop_front() {
            Some(msg) => Ok(msg),
            None => anyhow::bail!("in-memory wire stream closed"),
        }
    }

    async fn read_message_timeout(&mut self, timeout: Duration) -> Result<WireMessage> {
        match tokio::time::timeout(timeout, self.read_message()).await {
            Ok(msg) => msg,
            Err(_) => anyhow::bail!("Wire message read timed out after {:?}", timeout),
        }
    }

    async fn read_response<T: DeserializeOwned>(&mut self, expected_id: &str) -> Result<T> {
        loop {
            let idx = {
                let lock = self.pending.lock().await;
                lock.iter()
                    .position(|msg| wire_message_id(msg) == Some(expected_id))
            };
            if let Some(idx) = idx {
                let msg = self
                    .pending
                    .lock()
                    .await
                    .remove(idx)
                    .context("pending response index should be valid")?;
                return decode_response(msg, expected_id);
            }

            match self.incoming.lock().await.pop_front() {
                Some(WireMessage::SuccessResponse(resp)) if resp.id == expected_id => {
                    return serde_json::from_value(resp.result)
                        .context("Failed to decode response result");
                }
                Some(WireMessage::ErrorResponse(resp)) if resp.id == expected_id => {
                    return bail_wire_error(resp);
                }
                Some(other) => {
                    self.pending.lock().await.push_back(other);
                }
                None => anyhow::bail!("in-memory wire stream closed while waiting for response"),
            }
        }
    }

    async fn send_response<T: Serialize>(&mut self, id: &str, result: T) -> Result<()> {
        let resp = crate::wire::protocol::JsonRpcSuccessResponse {
            jsonrpc: "2.0".to_string(),
            id: id.to_string(),
            result,
        };
        let line = format!("{}\n", serde_json::to_string(&resp)?);
        self.outgoing
            .lock()
            .await
            .push(serde_json::Value::String(line));
        Ok(())
    }

    async fn send_error(&mut self, id: &str, code: i32, message: &str) -> Result<()> {
        let resp = crate::wire::protocol::JsonRpcErrorResponse {
            jsonrpc: "2.0".to_string(),
            id: id.to_string(),
            error: crate::wire::protocol::JsonRpcError {
                code,
                message: message.to_string(),
                data: None,
            },
        };
        let line = format!("{}\n", serde_json::to_string(&resp)?);
        self.outgoing
            .lock()
            .await
            .push(serde_json::Value::String(line));
        Ok(())
    }

    async fn initialize(&mut self, _params: InitializeParams) -> Result<InitializeResult> {
        self.handshake_done = true;
        Ok(InitializeResult {
            protocol_version: "test".to_string(),
            server: None,
            slash_commands: None,
            external_tools: None,
            capabilities: None,
            hooks: None,
        })
    }

    fn is_handshake_done(&self) -> bool {
        self.handshake_done
    }

    async fn shutdown(self) -> Result<()> {
        Ok(())
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

fn decode_response<T: DeserializeOwned>(msg: WireMessage, expected_id: &str) -> Result<T> {
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

fn bail_wire_error<T>(resp: JsonRpcErrorResponse) -> Result<T> {
    anyhow::bail!(
        "Wire request failed: {} (code: {})",
        resp.error.message,
        resp.error.code
    )
}
