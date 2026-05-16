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
