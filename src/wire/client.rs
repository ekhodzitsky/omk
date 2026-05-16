use anyhow::Context;
use std::collections::VecDeque;
use tokio::process::{Child, ChildStdin, ChildStdout};
use tokio_util::codec::{FramedRead, LinesCodec};

mod client_trait;
mod dispatch;
mod io;
mod process_impl;
mod spawn;

pub use client_trait::{InMemoryWireClient, WireClient};
pub use dispatch::{process_messages, WireMessage, WireResponse};

const LEGACY_NO_HANDSHAKE_PROTOCOL_VERSION: &str = "legacy/no-handshake";

/// Maximum wire-line length in bytes.
///
/// Each Kimi wire message arrives as a single newline-terminated JSON line.
/// Without a hard cap, a peer that never emits a newline can drive
/// `read_line` to allocate until OOM. 16 MiB is generous enough for any
/// realistic LLM turn (most are well under 100 KB) yet still bounds the
/// damage a misbehaving / hostile producer can cause. The same cap is used
/// for inbound MCP requests in `crate::mcp::server`.
pub(crate) const MAX_WIRE_LINE_LENGTH: usize = 16 * 1024 * 1024;

/// A client for communicating with Kimi Code CLI in wire mode via a child process.
#[derive(Debug)]
pub struct ProcessWireClient {
    pub(crate) child: Child,
    pub(crate) stdin: ChildStdin,
    pub(crate) stdout_reader: FramedRead<ChildStdout, LinesCodec>,
    pub(crate) pending_messages: VecDeque<WireMessage>,
    pub(crate) request_id_counter: u64,
    pub(crate) handshake_done: bool,
    pub(crate) stderr_handle: Option<tokio::task::JoinHandle<()>>,
}

impl Drop for ProcessWireClient {
    fn drop(&mut self) {
        if let Some(handle) = self.stderr_handle.take() {
            handle.abort();
        }
    }
}

#[cfg(test)]
mod tests;

// Shared helpers used by process_impl.rs and client_trait.rs
pub(crate) fn wire_message_id(msg: &WireMessage) -> Option<&str> {
    match msg {
        WireMessage::SuccessResponse(resp) => Some(resp.id.as_str()),
        WireMessage::ErrorResponse(resp) => Some(resp.id.as_str()),
        WireMessage::Request(req) => Some(req.id.as_str()),
        WireMessage::Event(_) => None,
    }
}

pub(crate) fn decode_response<T: serde::de::DeserializeOwned>(
    msg: WireMessage,
    expected_id: &str,
) -> anyhow::Result<T> {
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

pub(crate) fn bail_wire_error<T>(
    resp: crate::wire::protocol::JsonRpcErrorResponse,
) -> anyhow::Result<T> {
    anyhow::bail!(
        "Wire request failed: {} (code: {})",
        resp.error.message,
        resp.error.code
    )
}
