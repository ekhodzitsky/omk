use anyhow::{Context, Result};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tracing::{debug, info, warn};

#[derive(Debug)]
pub struct StdioMcpTransport {
    stdin: ChildStdin,
    stdout_reader: BufReader<ChildStdout>,
    child: Child,
    server_name: String,
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
        let mut child = cmd
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
            tokio::spawn(async move {
                let reader = BufReader::new(stderr);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    debug!(server = %name, stderr = %line, "MCP server stderr");
                }
            });
        }
        info!(server = %server_name, "MCP stdio transport spawned");
        Ok(Self {
            stdin,
            stdout_reader: BufReader::new(stdout),
            child,
            server_name,
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
        let mut line = String::new();
        match tokio::time::timeout(
            std::time::Duration::from_secs(60),
            self.stdout_reader.read_line(&mut line),
        )
        .await
        {
            Ok(Ok(0)) => {
                info!(server = %self.server_name, "MCP server stdout closed");
                Ok(None)
            }
            Ok(Ok(_)) => {
                let line = line.trim_end().to_string();
                if line.is_empty() {
                    return Ok(None);
                }
                debug!(server = %self.server_name, len = line.len(), "MCP transport recv");
                Ok(Some(line))
            }
            Ok(Err(e)) => {
                Err(anyhow::Error::new(e).context("failed to read from MCP server stdout"))
            }
            Err(_) => {
                warn!(server = %self.server_name, "MCP transport recv timeout");
                Err(anyhow::anyhow!("MCP transport recv timeout after 60s"))
            }
        }
    }

    pub async fn close(&mut self) -> Result<()> {
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
