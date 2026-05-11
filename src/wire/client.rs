use std::collections::VecDeque;
use tokio::io::BufReader;
use tokio::process::{Child, ChildStdin, ChildStdout};

mod dispatch;
mod io;
mod messaging;
mod spawn;

pub use dispatch::{process_messages, WireMessage, WireResponse};

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

#[cfg(test)]
mod tests;
