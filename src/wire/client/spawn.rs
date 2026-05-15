use anyhow::{Context, Result};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::time::Duration;
use tokio_stream::StreamExt;
use tokio_util::codec::{FramedRead, LinesCodec};
use tracing::{info, warn};

use crate::wire::client::{WireClient, LEGACY_NO_HANDSHAKE_PROTOCOL_VERSION, MAX_WIRE_LINE_LENGTH};
use crate::wire::protocol::{
    InitializeParams, InitializeResult, JsonRpcErrorResponse, JsonRpcRequest,
    JsonRpcSuccessResponse,
};

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
        // FramedRead with a length-capped LinesCodec: each line is bounded at
        // MAX_WIRE_LINE_LENGTH (16 MiB). Without the cap, a peer that omits
        // newlines can drive the reader to OOM the host.
        let stdout_reader =
            FramedRead::new(stdout, LinesCodec::new_with_max_length(MAX_WIRE_LINE_LENGTH));

        // Drain stderr in a background task so a verbose kimi cannot fill the
        // pipe buffer (typically 64 KiB) and block its own writes — which would
        // otherwise deadlock the wire session.
        if let Some(stderr) = child.stderr.take() {
            tokio::spawn(async move {
                let mut reader = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    warn!(target: "kimi.stderr", "{}", line);
                }
            });
        }

        info!("Wire client spawned");

        Ok(Self {
            child,
            stdin,
            stdout_reader,
            pending_messages: std::collections::VecDeque::new(),
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
}
