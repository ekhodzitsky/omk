use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::{debug, info, warn};

use crate::mcp::client::transport_trait::McpTransport;
use crate::mcp::client::types::{InitializeResult};
use super::types::{JsonRpcPayload, JsonRpcRequest, JsonRpcResponse, McpClient};

impl<T: McpTransport> McpClient<T> {
    pub fn new(transport: T, server_name: impl Into<String>) -> Self {
        Self {
            transport,
            request_id: 0,
            server_name: server_name.into(),
        }
    }

    fn next_id(&mut self) -> u64 {
        self.request_id += 1;
        self.request_id
    }

    pub(crate) async fn request<P: Serialize, R: for<'de> Deserialize<'de>>(
        &mut self,
        method: &str,
        params: Option<P>,
    ) -> Result<R> {
        let id = self.next_id();
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id,
            method: method.to_string(),
            params,
        };
        let req_json = serde_json::to_string(&req).with_context(|| {
            format!(
                "failed to serialize MCP {method} request for {}",
                self.server_name
            )
        })?;
        self.transport.send(req_json).await.with_context(|| {
            format!(
                "MCP transport send failed for {} method {method}",
                self.server_name
            )
        })?;

        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(60);
        loop {
            let line = match tokio::time::timeout_at(deadline, self.transport.recv()).await {
                Ok(Ok(Some(l))) => l,
                Ok(Ok(None)) => bail!("MCP server {} closed connection", self.server_name),
                Ok(Err(e)) => bail!("MCP transport recv error for {}: {e}", self.server_name),
                Err(_) => bail!(
                    "MCP request timeout for {method} on {} after 60s",
                    self.server_name
                ),
            };

            // Demux: skip notifications and responses that don't match our id.
            let value: Value = serde_json::from_str(&line).with_context(|| {
                format!(
                    "failed to parse MCP JSON from {} for method {method}: {line}",
                    self.server_name
                )
            })?;
            if value.get("id").is_none() {
                debug!(server = %self.server_name, line = %line, "MCP notification or unsolicited message, skipping");
                continue;
            }
            let resp: JsonRpcResponse<R> = serde_json::from_value(value).with_context(|| {
                format!(
                    "failed to parse MCP response from {} for method {method}: {line}",
                    self.server_name
                )
            })?;
            if resp.jsonrpc != "2.0" {
                bail!(
                    "MCP server {} returned unsupported JSON-RPC version {} for {}",
                    self.server_name,
                    resp.jsonrpc,
                    method
                );
            }
            if resp.id != id {
                warn!(server = %self.server_name, expected = id, got = resp.id, "MCP JSON-RPC id mismatch, skipping");
                continue;
            }
            match resp.payload {
                JsonRpcPayload::Result(result) => return Ok(result),
                JsonRpcPayload::Error(err) => {
                    if let Some(data) = err.data {
                        bail!(
                            "MCP server {} returned error for {}: {} (code: {}, data: {})",
                            self.server_name,
                            method,
                            err.message,
                            err.code,
                            data
                        );
                    }
                    bail!(
                        "MCP server {} returned error for {}: {} (code: {})",
                        self.server_name,
                        method,
                        err.message,
                        err.code
                    );
                }
            }
        }
    }

    pub async fn initialize(&mut self) -> Result<InitializeResult> {
        let params = serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {},
                "resources": {}
            },
            "clientInfo": {
                "name": "omk-mcp-client",
                "version": env!("CARGO_PKG_VERSION")
            }
        });
        let result: InitializeResult = self.request("initialize", Some(params)).await?;
        let notify = serde_json::json!({"jsonrpc": "2.0", "method": "notifications/initialized"});
        let notify_json =
            serde_json::to_string(&notify).context("serialize initialized notification")?;
        if let Err(e) = self.transport.send(notify_json).await {
            warn!(error = %e, "failed to send initialized notification");
        }
        info!(server = %self.server_name, version = %result.protocol_version, "MCP client initialized");
        Ok(result)
    }
}
