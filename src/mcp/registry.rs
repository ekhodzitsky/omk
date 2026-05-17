use super::client::transport::StdioMcpTransport;
use super::client::types::Tool;
use super::client::McpClient;
use super::config::{McpConfig, McpServerConfig};
use crate::error::OmkError;
use anyhow::{Context, Result};
use serde_json::Value;
use std::collections::HashMap;
use tracing::{debug, info, warn};

#[derive(Debug)]
struct McpServerHandle {
    name: String,
    client: McpClient,
    tools: Vec<Tool>,
}

#[derive(Debug)]
pub struct McpRegistry {
    servers: HashMap<String, McpServerHandle>,
}

impl McpRegistry {
    pub fn new() -> Self {
        Self {
            servers: HashMap::new(),
        }
    }

    pub async fn from_config(config: &McpConfig) -> Result<Self> {
        let mut registry = Self::new();
        for (name, server_config) in &config.servers {
            match registry.start_server(name.clone(), server_config).await {
                Ok(()) => info!(server = %name, "MCP server started and tools discovered"),
                Err(e) => warn!(server = %name, error = %e, "Failed to start MCP server, skipping"),
            }
        }
        Ok(registry)
    }

    async fn start_server(&mut self, name: String, config: &McpServerConfig) -> Result<()> {
        let transport = StdioMcpTransport::spawn(&name, &config.command, &config.args, &config.env)
            .with_context(|| format!("failed to spawn MCP server '{name}'"))?;
        let mut client = McpClient::new(transport, name.clone());
        client
            .initialize()
            .await
            .with_context(|| format!("MCP initialize failed for server '{name}'"))?;
        let tools = client
            .list_tools()
            .await
            .with_context(|| format!("MCP list_tools failed for server '{name}'"))?;
        self.servers.insert(
            name.clone(),
            McpServerHandle {
                name,
                client,
                tools,
            },
        );
        Ok(())
    }

    pub fn all_tools(&self) -> Vec<(&str, &Tool)> {
        let mut out = Vec::new();
        for handle in self.servers.values() {
            for tool in &handle.tools {
                out.push((handle.name.as_str(), tool));
            }
        }
        out
    }

    pub fn find_server_for_tool(&self, tool_name: &str) -> Option<&str> {
        for handle in self.servers.values() {
            if handle.tools.iter().any(|t| t.name == tool_name) {
                return Some(&handle.name);
            }
        }
        None
    }

    pub async fn call_tool(
        &mut self,
        tool_name: &str,
        arguments: Value,
    ) -> Result<Value, OmkError> {
        let server_name = self
            .find_server_for_tool(tool_name)
            .ok_or_else(|| OmkError::InvalidInput {
                reason: format!("MCP tool '{tool_name}' not found in any registered server"),
            })?
            .to_string();
        let handle = self
            .servers
            .get_mut(&server_name)
            .ok_or_else(|| OmkError::McpTransport {
                server: server_name.clone(),
                reason: "server handle missing".to_string(),
            })?;
        let result = handle
            .client
            .call_tool(tool_name, arguments)
            .await
            .map_err(|e| OmkError::McpToolCall {
                server: server_name.clone(),
                tool: tool_name.to_string(),
                reason: e.to_string(),
            })?;
        let mut texts = Vec::new();
        let mut is_error = false;
        for content in &result.content {
            match content {
                super::client::types::ToolContent::Text { text } => texts.push(text.clone()),
                super::client::types::ToolContent::Unknown => {}
            }
        }
        if result.is_error == Some(true) {
            is_error = true;
        }
        let output = serde_json::json!({"texts": texts, "is_error": is_error});
        debug!(server = %server_name, tool = %tool_name, "MCP tool call completed");
        Ok(output)
    }

    pub fn server_count(&self) -> usize {
        self.servers.len()
    }

    pub async fn call_tool_on_server(
        &mut self,
        server_name: &str,
        tool_name: &str,
        arguments: Value,
    ) -> Result<Value, OmkError> {
        let handle = self
            .servers
            .get_mut(server_name)
            .ok_or_else(|| OmkError::InvalidInput {
                reason: format!("MCP server '{server_name}' not found"),
            })?;
        if !handle.tools.iter().any(|t| t.name == tool_name) {
            return Err(OmkError::InvalidInput {
                reason: format!("MCP tool '{tool_name}' not found on server '{server_name}'"),
            });
        }
        let result = handle
            .client
            .call_tool(tool_name, arguments)
            .await
            .map_err(|e| OmkError::McpToolCall {
                server: server_name.to_string(),
                tool: tool_name.to_string(),
                reason: e.to_string(),
            })?;
        let mut texts = Vec::new();
        let mut is_error = false;
        for content in &result.content {
            match content {
                super::client::types::ToolContent::Text { text } => texts.push(text.clone()),
                super::client::types::ToolContent::Unknown => {}
            }
        }
        if result.is_error == Some(true) {
            is_error = true;
        }
        let output = serde_json::json!({"texts": texts, "is_error": is_error});
        debug!(server = %server_name, tool = %tool_name, "MCP tool call completed");
        Ok(output)
    }

    pub async fn shutdown_all(mut self) -> Result<()> {
        for (name, handle) in self.servers.drain() {
            if let Err(e) = handle.client.shutdown().await {
                warn!(server = %name, error = %e, "MCP server shutdown error");
            } else {
                debug!(server = %name, "MCP server shut down");
            }
        }
        Ok(())
    }
}

impl Default for McpRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for McpRegistry {
    fn drop(&mut self) {
        // Best-effort kill: draining servers drops McpClient -> StdioMcpTransport -> Child::start_kill.
        // Graceful async shutdown should be done via shutdown_all().await before drop.
        for (_, handle) in self.servers.drain() {
            drop(handle.client);
        }
    }
}
