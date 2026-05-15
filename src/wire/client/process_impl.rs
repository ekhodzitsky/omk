use anyhow::{Context, Result};
use serde::{de::DeserializeOwned, Serialize};
use tokio::io::AsyncWriteExt;
use tokio::time::Duration;
use tokio_stream::StreamExt;
use tracing::{debug, warn};

use crate::wire::client::{
    bail_wire_error, decode_response, wire_message_id, ProcessWireClient, WireClient, WireMessage,
};
use crate::wire::protocol::{
    InitializeParams, InitializeResult, JsonRpcErrorResponse, JsonRpcRequest,
    JsonRpcSuccessResponse,
};

impl WireClient for ProcessWireClient {
    async fn read_message(&mut self) -> Result<WireMessage> {
        if let Some(msg) = self.pending_messages.pop_front() {
            return Ok(msg);
        }
        self.read_message_from_stdout().await
    }

    async fn read_message_timeout(&mut self, timeout: Duration) -> Result<WireMessage> {
        match tokio::time::timeout(timeout, self.read_message()).await {
            Ok(msg) => msg,
            Err(_) => anyhow::bail!("Wire message read timed out after {:?}", timeout),
        }
    }

    async fn shutdown(mut self) -> Result<()> {
        let _ = self.child.start_kill();
        let _ = self.child.wait().await;
        Ok(())
    }

    fn is_handshake_done(&self) -> bool {
        self.handshake_done
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
                    .ok_or_else(|| anyhow::anyhow!("pending response index should be valid"))?;
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

    async fn send_response<ResultType: Serialize>(
        &mut self,
        id: &str,
        result: ResultType,
    ) -> Result<()> {
        let resp = crate::wire::protocol::JsonRpcSuccessResponse {
            jsonrpc: "2.0".to_string(),
            id: id.to_string(),
            result,
        };
        let line = format!("{}\n", serde_json::to_string(&resp)?);
        self.stdin.write_all(line.as_bytes()).await?;
        self.stdin.flush().await?;
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
        self.stdin.write_all(line.as_bytes()).await?;
        self.stdin.flush().await?;
        Ok(())
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

    async fn initialize(&mut self, params: InitializeParams) -> Result<InitializeResult> {
        let id = self.next_id();
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "initialize".to_string(),
            id: id.clone(),
            params,
        };
        self.send_request(&req).await?;

        let line = match self.stdout_reader.next().await {
            Some(Ok(line)) => line,
            Some(Err(e)) => {
                return Err(e).context("Failed to read initialize response");
            }
            None => {
                anyhow::bail!("kimi stdout closed while waiting for initialize response");
            }
        };

        // Check for method-not-found error (-32601)
        if let Ok(error_resp) = serde_json::from_str::<JsonRpcErrorResponse>(&line) {
            if error_resp.error.code == -32601 {
                warn!(
                    code = error_resp.error.code,
                    "Server does not support initialize, falling back to legacy no-handshake mode"
                );
                self.handshake_done = true;
                return Ok(InitializeResult {
                    protocol_version: crate::wire::client::LEGACY_NO_HANDSHAKE_PROTOCOL_VERSION
                        .to_string(),
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
}
