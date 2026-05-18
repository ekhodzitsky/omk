use super::transport_trait::McpTransport;
use crate::wire::protocol::scrub_secret_patterns;
use anyhow::{Context, Result};
use std::future::Future;
use std::pin::Pin;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

#[derive(Debug)]
pub struct StdioMcpTransport {
    stdin: ChildStdin,
    lines_rx: tokio::sync::mpsc::UnboundedReceiver<String>,
    child: Child,
    server_name: String,
    cancel_token: CancellationToken,
}

impl StdioMcpTransport {
    pub fn spawn(
        server_name: impl Into<String>,
        command: &str,
        args: &[String],
        env: &std::collections::HashMap<String, String>,
    ) -> Result<Self> {
        let server_name = server_name.into();
        let mut cmd = Command::new(command);
        cmd.args(args);
        for (k, v) in env {
            cmd.env(k, v);
        }
        cmd.stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        let cancel_token = CancellationToken::new();
        let mut child = cmd
            .kill_on_drop(true)
            .spawn()
            .with_context(|| format!("failed to spawn MCP server '{}' ({command})", server_name))?;
        let stdin = child
            .stdin
            .take()
            .context("MCP server stdin not available")?;
        let stdout = child
            .stdout
            .take()
            .context("MCP server stdout not available")?;
        if let Some(stderr) = child.stderr.take() {
            let name = server_name.clone();
            let cancel = cancel_token.child_token();
            tokio::spawn(async move {
                let reader = BufReader::new(stderr);
                let mut lines = reader.lines();
                loop {
                    tokio::select! {
                        biased;
                        _ = cancel.cancelled() => break,
                        result = lines.next_line() => {
                            match result {
                                Ok(Some(line)) => debug!(server = %name, stderr = %scrub_secret_patterns(&line), "MCP server stderr"),
                                _ => break,
                            }
                        }
                    }
                }
            });
        }

        let (lines_tx, lines_rx) = tokio::sync::mpsc::unbounded_channel();
        let name = server_name.clone();
        let cancel = cancel_token.child_token();
        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout);
            let mut buf = String::new();
            loop {
                if cancel.is_cancelled() {
                    break;
                }
                buf.clear();
                match tokio::time::timeout(
                    std::time::Duration::from_secs(60),
                    reader.read_line(&mut buf),
                )
                .await
                {
                    Ok(Ok(0)) => break,
                    Ok(Ok(_)) => {
                        let line = buf.trim_end().to_string();
                        if line.is_empty() {
                            continue;
                        }
                        if lines_tx.send(line).is_err() {
                            break;
                        }
                    }
                    Ok(Err(e)) => {
                        warn!(server = %name, error = %e, "MCP stdout read error");
                        break;
                    }
                    Err(_) => {
                        warn!(server = %name, "MCP stdout read timeout");
                        break;
                    }
                }
            }
            debug!(server = %name, "MCP stdout reader task ended");
        });

        info!(server = %server_name, "MCP stdio transport spawned");
        Ok(Self {
            stdin,
            lines_rx,
            child,
            server_name,
            cancel_token,
        })
    }

    pub async fn send(&mut self, message: String) -> Result<()> {
        self.stdin
            .write_all(message.as_bytes())
            .await
            .context("failed to write to MCP server stdin")?;
        self.stdin
            .write_all(b"\n")
            .await
            .context("failed to write newline to MCP server stdin")?;
        self.stdin
            .flush()
            .await
            .context("failed to flush MCP server stdin")?;
        debug!(server = %self.server_name, len = message.len(), "MCP transport send");
        Ok(())
    }

    pub async fn recv(&mut self) -> Result<Option<String>> {
        match self.lines_rx.recv().await {
            Some(line) => {
                debug!(server = %self.server_name, len = line.len(), "MCP transport recv");
                Ok(Some(line))
            }
            None => {
                info!(server = %self.server_name, "MCP transport line channel closed");
                Ok(None)
            }
        }
    }

    pub async fn close(&mut self) -> Result<()> {
        self.cancel_token.cancel();
        match self.child.start_kill() {
            Ok(()) => {
                let _ = tokio::time::timeout(std::time::Duration::from_secs(5), self.child.wait())
                    .await;
            }
            Err(e) => warn!(error = %e, "failed to start_kill MCP child"),
        }
        Ok(())
    }
}

impl McpTransport for StdioMcpTransport {
    fn send(&mut self, message: String) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        Box::pin(async move { self.send(message).await })
    }

    fn recv(&mut self) -> Pin<Box<dyn Future<Output = Result<Option<String>>> + Send + '_>> {
        Box::pin(async move { self.recv().await })
    }

    fn close(&mut self) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        Box::pin(async move { self.close().await })
    }
}

impl Drop for StdioMcpTransport {
    fn drop(&mut self) {
        // Best-effort kill so the child does not outlive the transport.
        // Graceful shutdown should be done via close().await before drop.
        let _ = self.child.start_kill();
    }
}
