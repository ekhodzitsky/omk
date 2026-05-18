use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::Value;
use tracing::debug;

use super::types::McpClient;
use crate::mcp::client::transport_trait::McpTransport;
use crate::mcp::client::types::{CallToolResult, Resource, ResourceContent, Tool};

impl<T: McpTransport> McpClient<T> {
    pub async fn list_tools(&mut self) -> Result<Vec<Tool>> {
        #[derive(Debug, Deserialize)]
        struct ListToolsResult {
            tools: Vec<Tool>,
        }
        let result: ListToolsResult = self.request("tools/list", None::<Value>).await?;
        debug!(server = %self.server_name, count = result.tools.len(), "MCP tools listed");
        Ok(result.tools)
    }

    pub async fn call_tool(&mut self, name: &str, arguments: Value) -> Result<CallToolResult> {
        let params = serde_json::json!({"name": name, "arguments": arguments});
        let result: CallToolResult = self
            .request("tools/call", Some(params))
            .await
            .with_context(|| format!("MCP tool call failed: {name} on {}", self.server_name))?;
        Ok(result)
    }

    pub async fn list_resources(&mut self) -> Result<Vec<Resource>> {
        #[derive(Debug, Deserialize)]
        struct ListResourcesResult {
            resources: Vec<Resource>,
        }
        let result: ListResourcesResult = self.request("resources/list", None::<Value>).await?;
        debug!(server = %self.server_name, count = result.resources.len(), "MCP resources listed");
        Ok(result.resources)
    }

    pub async fn read_resource(&mut self, uri: &str) -> Result<Vec<ResourceContent>> {
        #[derive(Debug, Deserialize)]
        struct ReadResourceResult {
            contents: Vec<ResourceContent>,
        }
        let params = serde_json::json!({"uri": uri});
        let result: ReadResourceResult = self
            .request("resources/read", Some(params))
            .await
            .with_context(|| {
                format!("MCP read_resource failed for {uri} on {}", self.server_name)
            })?;
        Ok(result.contents)
    }

    pub async fn shutdown(mut self) -> Result<()> {
        self.transport.close().await
    }

    pub fn server_name(&self) -> &str {
        &self.server_name
    }

    pub fn into_transport(self) -> T {
        self.transport
    }
}
