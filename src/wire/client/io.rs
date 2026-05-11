use anyhow::{Context, Result};
use serde::de::DeserializeOwned;
use tokio::io::AsyncBufReadExt;
use tokio::time::Duration;
use tracing::debug;

use crate::wire::client::WireClient;
use crate::wire::client::WireMessage;
use crate::wire::protocol::JsonRpcErrorResponse;

impl WireClient {
    /// Read the next message from stdout (event, request, or response).
    pub async fn read_message(&mut self) -> Result<WireMessage> {
        if let Some(msg) = self.pending_messages.pop_front() {
            return Ok(msg);
        }

        self.read_message_from_stdout().await
    }

    pub(super) async fn read_message_from_stdout(&mut self) -> Result<WireMessage> {
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

    /// Gracefully shutdown the child process.
    pub async fn shutdown(mut self) -> Result<()> {
        let _ = self.child.kill().await;
        Ok(())
    }

    /// Returns true if the initialize handshake has completed (or was skipped via fallback).
    pub fn is_handshake_done(&self) -> bool {
        self.handshake_done
    }

    pub(super) async fn read_response<ResultType: DeserializeOwned>(
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
