use std::collections::VecDeque;
use tokio::process::{Child, ChildStdin, ChildStdout};
use tokio_util::codec::{FramedRead, LinesCodec};

mod dispatch;
mod io;
mod messaging;
mod spawn;

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

/// A client for communicating with Kimi Code CLI in wire mode.
pub struct WireClient {
    child: Child,
    stdin: ChildStdin,
    stdout_reader: FramedRead<ChildStdout, LinesCodec>,
    pending_messages: VecDeque<WireMessage>,
    request_id_counter: u64,
    handshake_done: bool,
}

#[cfg(test)]
mod tests;
