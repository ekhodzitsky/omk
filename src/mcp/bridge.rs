use super::registry::McpRegistry;
use crate::error::OmkError;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::debug;

#[derive(Debug, Clone)]
pub struct WireWorkerMcpBridge {
    registry: Arc<RwLock<McpRegistry>>,
}

impl WireWorkerMcpBridge {
    pub fn new(registry: Arc<RwLock<McpRegistry>>) -> Self {
        Self { registry }
    }

    pub async fn external_tools(&self) -> Vec<Value> {
        let registry = self.registry.read().await;
        let mut tools = Vec::new();
        for (_server, tool) in registry.all_tools() {
            let mut obj = serde_json::json!({"name": tool.name});
            if let Some(desc) = &tool.description {
                obj["description"] = serde_json::json!(desc);
            }
            if let Some(schema) = &tool.input_schema {
                obj["parameters"] = schema.clone();
            }
            tools.push(obj);
        }
        debug!(
            count = tools.len(),
            "Built external_tools for Wire initialize"
        );
        tools
    }

    pub async fn is_mcp_tool(&self, tool_name: &str) -> bool {
        let registry = self.registry.read().await;
        registry.find_server_for_tool(tool_name).is_some()
    }

    pub async fn handle_tool_call(
        &self,
        tool_name: &str,
        arguments: Value,
    ) -> Result<Value, OmkError> {
        let mut registry = self.registry.write().await;
        registry.call_tool(tool_name, arguments).await
    }
}

/// Attempt to create an MCP bridge from the default config path.
/// Returns `None` if no MCP servers are configured or initialization fails.
pub async fn maybe_create_bridge() -> Option<Arc<WireWorkerMcpBridge>> {
    let config_path = super::config::McpConfig::default_path();
    let mcp_config = match super::config::McpConfig::load(&config_path).await {
        Ok(cfg) => cfg,
        Err(e) => {
            tracing::debug!(error = %e, "MCP config not loaded, skipping bridge");
            return None;
        }
    };
    if mcp_config.servers.is_empty() {
        return None;
    }
    match McpRegistry::from_config(&mcp_config).await {
        Ok(registry) => {
            let bridge = WireWorkerMcpBridge::new(Arc::new(RwLock::new(registry)));
            Some(Arc::new(bridge))
        }
        Err(e) => {
            tracing::warn!(error = %e, "Failed to initialize MCP registry, continuing without bridge");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::client::transport_trait::McpTransport;
    use crate::mcp::client::types::Tool;
    use crate::mcp::client::McpClient;
    use crate::mcp::registry::McpServerHandle;
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
        ) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + '_>> {
            Box::pin(async move { Ok(()) })
        }

        fn recv(
            &mut self,
        ) -> Pin<Box<dyn Future<Output = anyhow::Result<Option<String>>> + Send + '_>> {
            let responses = self.responses.clone();
            Box::pin(async move { Ok(responses.lock().unwrap().pop_front()) })
        }

        fn close(&mut self) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + '_>> {
            Box::pin(async move { Ok(()) })
        }
    }

    fn make_registry_with_tools(tools: Vec<Tool>) -> McpRegistry {
        let mut registry = McpRegistry::new();
        let transport: Box<dyn McpTransport> = Box::new(MockTransport::new(vec![]));
        let client = McpClient::new(transport, "server-a");
        registry.servers.insert(
            "server-a".to_string(),
            McpServerHandle {
                name: "server-a".to_string(),
                client,
                tools,
            },
        );
        registry
    }

    #[tokio::test]
    async fn test_bridge_external_tools() {
        let registry = make_registry_with_tools(vec![Tool {
            name: "tool-a".to_string(),
            description: Some("desc-a".to_string()),
            input_schema: Some(serde_json::json!({"type": "object"})),
        }]);
        let bridge = WireWorkerMcpBridge::new(Arc::new(RwLock::new(registry)));
        let tools = bridge.external_tools().await;
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0]["name"], "tool-a");
        assert_eq!(tools[0]["description"], "desc-a");
        assert_eq!(
            tools[0]["parameters"],
            serde_json::json!({"type": "object"})
        );
    }

    #[tokio::test]
    async fn test_bridge_is_mcp_tool() {
        let registry = make_registry_with_tools(vec![Tool {
            name: "tool-a".to_string(),
            description: None,
            input_schema: None,
        }]);
        let bridge = WireWorkerMcpBridge::new(Arc::new(RwLock::new(registry)));
        assert!(bridge.is_mcp_tool("tool-a").await);
        assert!(!bridge.is_mcp_tool("missing").await);
    }

    #[tokio::test]
    async fn test_bridge_handle_tool_call() {
        let mut registry = McpRegistry::new();
        let call_resp = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "content": [{"type": "text", "text": "hello"}],
                "isError": false
            }
        })
        .to_string();
        let transport: Box<dyn McpTransport> = Box::new(MockTransport::new(vec![call_resp]));
        let client = McpClient::new(transport, "server-a");
        registry.servers.insert(
            "server-a".to_string(),
            McpServerHandle {
                name: "server-a".to_string(),
                client,
                tools: vec![Tool {
                    name: "greet".to_string(),
                    description: None,
                    input_schema: None,
                }],
            },
        );
        let bridge = WireWorkerMcpBridge::new(Arc::new(RwLock::new(registry)));
        let result = bridge
            .handle_tool_call("greet", serde_json::json!({"name": "world"}))
            .await
            .unwrap();
        assert_eq!(result["texts"], serde_json::json![["hello"]]);
    }

    #[tokio::test]
    async fn test_bridge_tool_not_found() {
        let registry = make_registry_with_tools(vec![]);
        let bridge = WireWorkerMcpBridge::new(Arc::new(RwLock::new(registry)));
        let result = bridge
            .handle_tool_call("missing", serde_json::json!({}))
            .await;
        assert!(result.is_err());
    }
}
