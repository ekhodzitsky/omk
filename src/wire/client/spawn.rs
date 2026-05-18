use anyhow::{Context, Result};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::time::Duration;
use tokio_util::codec::{FramedRead, LinesCodec};
use tracing::{info, warn};

use crate::wire::client::{ProcessWireClient, MAX_WIRE_LINE_LENGTH};

impl ProcessWireClient {
    /// Spawn a new kimi process in wire mode.
    pub async fn spawn(
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
                    tokio::time::sleep(Duration::from_millis(25)).await;
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
        let stdout_reader = FramedRead::new(
            stdout,
            LinesCodec::new_with_max_length(MAX_WIRE_LINE_LENGTH),
        );

        // Drain stderr in a background task so a verbose kimi cannot fill the
        // pipe buffer (typically 64 KiB) and block its own writes — which would
        // otherwise deadlock the wire session.
        let stderr_handle = child.stderr.take().map(|stderr| {
            tokio::spawn(async move {
                let mut reader = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    warn!(target: "kimi.stderr", "{}", line);
                }
            })
        });

        info!("Wire client spawned");

        Ok(Self {
            child,
            stdin,
            stdout_reader,
            pending_messages: std::collections::VecDeque::new(),
            request_id_counter: 0,
            handshake_done: false,
            stderr_handle,
        })
    }
}
