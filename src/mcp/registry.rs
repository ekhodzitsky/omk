use super::client::transport::StdioMcpTransport;
use super::client::transport_trait::McpTransport;
use super::client::types::{CallToolResult, Tool};
use super::client::McpClient;
use super::config::{McpConfig, McpServerConfig, TransportType};
use crate::error::OmkError;
use anyhow::{Context, Result};
use moka::future::Cache;
use serde_json::Value;
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use tracing::{debug, info, warn};

#[derive(Debug)]
pub(crate) struct McpServerHandle {
    pub(crate) name: String,
    pub(crate) client: McpClient<Box<dyn McpTransport>>,
    pub(crate) tool_cache: Cache<String, CallToolResult>,
    pub(crate) tools: Vec<Tool>,
}

#[derive(Debug)]
pub struct McpRegistry {
    pub(crate) servers: HashMap<String, McpServerHandle>,
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
        let transport: Box<dyn McpTransport> = match &config.transport {
            TransportType::Stdio { command, args, env } => Box::new(
                StdioMcpTransport::spawn(&name, command, args, env)
                    .with_context(|| format!("failed to spawn MCP server '{name}'"))?,
            ),
            TransportType::SseHttp { url, headers } => Box::new(
                super::client::http_transport::HttpMcpTransport::new(url, headers.clone())
                    .with_context(|| format!("failed to create HTTP MCP transport for '{name}'"))?,
            ),
        };
        let mut client = McpClient::new(transport, name.clone());
        client
            .initialize()
            .await
            .with_context(|| format!("MCP initialize failed for server '{name}'"))?;
        let tools = client
            .list_tools()
            .await
            .with_context(|| format!("MCP list_tools failed for server '{name}'"))?;
        let tool_cache = Cache::builder()
            .max_capacity(1000)
            .time_to_live(std::time::Duration::from_secs(300))
            .build();
        self.servers.insert(
            name.clone(),
            McpServerHandle {
                name,
                client,
                tool_cache,
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
        let key =
            cache_key(&server_name, tool_name, &arguments).map_err(|e| OmkError::McpToolCall {
                server: server_name.clone(),
                tool: tool_name.to_string(),
                reason: e.to_string(),
            })?;
        if let Some(cached) = handle.tool_cache.get(&key).await {
            debug!(server = %server_name, tool = %tool_name, "MCP tool call cache hit");
            return Ok(tool_result_to_json(cached));
        }
        let result = handle
            .client
            .call_tool(tool_name, arguments)
            .await
            .map_err(|e| OmkError::McpToolCall {
                server: server_name.clone(),
                tool: tool_name.to_string(),
                reason: e.to_string(),
            })?;
        handle.tool_cache.insert(key, result.clone()).await;
        debug!(server = %server_name, tool = %tool_name, "MCP tool call cache miss");
        Ok(tool_result_to_json(result))
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
        let key =
            cache_key(server_name, tool_name, &arguments).map_err(|e| OmkError::McpToolCall {
                server: server_name.to_string(),
                tool: tool_name.to_string(),
                reason: e.to_string(),
            })?;
        if let Some(cached) = handle.tool_cache.get(&key).await {
            debug!(server = %server_name, tool = %tool_name, "MCP tool call cache hit");
            return Ok(tool_result_to_json(cached));
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
        handle.tool_cache.insert(key, result.clone()).await;
        debug!(server = %server_name, tool = %tool_name, "MCP tool call cache miss");
        Ok(tool_result_to_json(result))
    }

    pub fn server_count(&self) -> usize {
        self.servers.len()
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

/// Generates a cache key from server name, tool name, and arguments.
fn cache_key(server_name: &str, tool_name: &str, arguments: &Value) -> Result<String> {
    let args_str =
        serde_json::to_string(arguments).context("failed to serialize arguments for cache key")?;
    let mut hasher = DefaultHasher::new();
    (server_name, tool_name, args_str).hash(&mut hasher);
    Ok(format!("{:x}", hasher.finish()))
}

/// Converts a [`CallToolResult`] into the JSON shape returned by the registry.
fn tool_result_to_json(result: CallToolResult) -> Value {
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
    serde_json::json!({"texts": texts, "is_error": is_error})
}

impl Default for McpRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for McpRegistry {
    fn drop(&mut self) {
        for (_, handle) in self.servers.drain() {
            drop(handle.client);
            drop(handle.tool_cache);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::client::transport_trait::McpTransport;
    use std::collections::VecDeque;
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::{Arc, Mutex};

    #[derive(Debug)]
    struct MockTransport {
        responses: Arc<Mutex<VecDeque<String>>>,
    }

    impl MockTransport {
        fn new(responses: Vec<String>) -> Self {
            Self {
                responses: Arc::new(Mutex::new(responses.into_iter().collect())),
            }
        }
    }

    impl McpTransport for MockTransport {
        fn send(
            &mut self,
            _message: String,
        ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
            Box::pin(async move { Ok(()) })
        }

        fn recv(&mut self) -> Pin<Box<dyn Future<Output = Result<Option<String>>> + Send + '_>> {
            let responses = self.responses.clone();
            Box::pin(async move { Ok(responses.lock().unwrap().pop_front()) })
        }

        fn close(&mut self) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
            Box::pin(async move { Ok(()) })
        }
    }

    fn make_init_response(id: u64) -> String {
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "protocolVersion": "2024-11-05",
                "serverInfo": {"name": "test", "version": "1.0"},
                "capabilities": {}
            }
        })
        .to_string()
    }

    fn make_tools_response(id: u64, tools: Vec<Tool>) -> String {
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {"tools": tools}
        })
        .to_string()
    }

    #[tokio::test]
    async fn test_registry_routing() {
        let mut registry = McpRegistry::new();

        let init = make_init_response(1);
        let tools = make_tools_response(
            2,
            vec![Tool {
                name: "tool-a".to_string(),
                description: None,
                input_schema: None,
            }],
        );
        let transport: Box<dyn McpTransport> = Box::new(MockTransport::new(vec![init, tools]));
        let client = McpClient::new(transport, "server-a");
        let tool_cache = Cache::builder()
            .max_capacity(1000)
            .time_to_live(std::time::Duration::from_secs(300))
            .build();
        registry.servers.insert(
            "server-a".to_string(),
            McpServerHandle {
                name: "server-a".to_string(),
                client,
                tool_cache,
                tools: vec![Tool {
                    name: "tool-a".to_string(),
                    description: None,
                    input_schema: None,
                }],
            },
        );

        assert_eq!(registry.server_count(), 1);
        assert_eq!(registry.find_server_for_tool("tool-a"), Some("server-a"));
        assert_eq!(registry.find_server_for_tool("missing"), None);

        let all = registry.all_tools();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].0, "server-a");
        assert_eq!(all[0].1.name, "tool-a");
    }

    #[tokio::test]
    async fn test_registry_call_tool() {
        let mut registry = McpRegistry::new();

        let call_resp = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "content": [{"type": "text", "text": "result"}],
                "isError": false
            }
        })
        .to_string();
        let transport: Box<dyn McpTransport> = Box::new(MockTransport::new(vec![call_resp]));
        let client = McpClient::new(transport, "server-a");
        let tool_cache = Cache::builder()
            .max_capacity(1000)
            .time_to_live(std::time::Duration::from_secs(300))
            .build();
        registry.servers.insert(
            "server-a".to_string(),
            McpServerHandle {
                name: "server-a".to_string(),
                client,
                tool_cache,
                tools: vec![Tool {
                    name: "tool-a".to_string(),
                    description: None,
                    input_schema: None,
                }],
            },
        );

        let result = registry
            .call_tool("tool-a", serde_json::json!({}))
            .await
            .unwrap();
        assert_eq!(result["texts"], serde_json::json![["result"]]);
        assert_eq!(result["is_error"], false);
    }

    #[tokio::test]
    async fn test_registry_tool_not_found() {
        let mut registry = McpRegistry::new();
        let result = registry.call_tool("missing", serde_json::json!({})).await;
        assert!(matches!(result, Err(OmkError::InvalidInput { .. })));
    }
}
