use super::transport_trait::McpTransport;
use super::types::{CallToolResult, InitializeResult, Resource, ResourceContent, Tool};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::{debug, info, warn};

pub struct McpClient<T: McpTransport> {
    transport: T,
    request_id: u64,
    server_name: String,
}

impl<T: McpTransport> std::fmt::Debug for McpClient<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpClient")
            .field("transport", &"<dyn McpTransport>")
            .field("request_id", &self.request_id)
            .field("server_name", &self.server_name)
            .finish()
    }
}

#[derive(Debug, Serialize)]
struct JsonRpcRequest<P> {
    jsonrpc: String,
    id: u64,
    method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<P>,
}

#[derive(Debug, Deserialize)]
struct JsonRpcResponse<R> {
    jsonrpc: String,
    id: u64,
    #[serde(flatten)]
    payload: JsonRpcPayload<R>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum JsonRpcPayload<R> {
    Result(R),
    Error(JsonRpcError),
}

#[derive(Debug, Deserialize, Clone)]
struct JsonRpcError {
    code: i32,
    message: String,
    #[serde(default)]
    data: Option<Value>,
}

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

    async fn request<P: Serialize, R: for<'de> Deserialize<'de>>(
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
        let line = match self.transport.recv().await {
            Ok(Some(l)) => l,
            Ok(None) => bail!("MCP server {} closed connection", self.server_name),
            Err(e) => bail!("MCP transport recv error for {}: {e}", self.server_name),
        };
        let resp: JsonRpcResponse<R> = serde_json::from_str(&line).with_context(|| {
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
            warn!(server = %self.server_name, expected = id, got = resp.id, "MCP JSON-RPC id mismatch");
        }
        match resp.payload {
            JsonRpcPayload::Result(result) => Ok(result),
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::{Arc, Mutex};

    #[derive(Debug)]
    struct MockTransport {
        sent: Arc<Mutex<Vec<String>>>,
        responses: Arc<Mutex<VecDeque<String>>>,
        closed: Arc<std::sync::atomic::AtomicBool>,
    }

    impl MockTransport {
        fn new(responses: Vec<String>) -> Self {
            Self {
                sent: Arc::new(Mutex::new(Vec::new())),
                responses: Arc::new(Mutex::new(responses.into_iter().collect())),
                closed: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            }
        }
    }

    impl McpTransport for MockTransport {
        fn send(
            &mut self,
            message: String,
        ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
            self.sent.lock().unwrap().push(message);
            Box::pin(async move { Ok(()) })
        }

        fn recv(&mut self) -> Pin<Box<dyn Future<Output = Result<Option<String>>> + Send + '_>> {
            let responses = self.responses.clone();
            Box::pin(async move { Ok(responses.lock().unwrap().pop_front()) })
        }

        fn close(&mut self) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
            self.closed.store(true, std::sync::atomic::Ordering::SeqCst);
            Box::pin(async move { Ok(()) })
        }
    }

    #[tokio::test]
    async fn test_initialize() {
        let init_response = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "protocolVersion": "2024-11-05",
                "serverInfo": {"name": "test", "version": "1.0"},
                "capabilities": {}
            }
        })
        .to_string();
        let transport = MockTransport::new(vec![init_response]);
        let mut client = McpClient::new(transport, "test");
        let result = client.initialize().await.unwrap();
        assert_eq!(result.protocol_version, "2024-11-05");
        assert_eq!(result.server_info.name, "test");
    }

    #[tokio::test]
    async fn test_list_tools() {
        let init_response = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "protocolVersion": "2024-11-05",
                "serverInfo": {"name": "test", "version": "1.0"},
                "capabilities": {}
            }
        })
        .to_string();
        let tools_response = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "result": {
                "tools": [
                    {"name": "tool-a", "description": "does a"},
                    {"name": "tool-b"}
                ]
            }
        })
        .to_string();
        let transport = MockTransport::new(vec![init_response, tools_response]);
        let mut client = McpClient::new(transport, "test");
        client.initialize().await.unwrap();
        let tools = client.list_tools().await.unwrap();
        assert_eq!(tools.len(), 2);
        assert_eq!(tools[0].name, "tool-a");
        assert_eq!(tools[0].description, Some("does a".to_string()));
    }

    #[tokio::test]
    async fn test_call_tool() {
        let call_response = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "content": [{"type": "text", "text": "hello"}],
                "isError": false
            }
        })
        .to_string();
        let transport = MockTransport::new(vec![call_response]);
        let mut client = McpClient::new(transport, "test");
        let result = client
            .call_tool("greet", serde_json::json!({"name": "world"}))
            .await
            .unwrap();
        assert_eq!(result.content.len(), 1);
        assert!(!result.is_error.unwrap_or(false));
    }

    #[tokio::test]
    async fn test_list_resources() {
        let res_response = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "resources": [
                    {"uri": "file:///tmp/a", "name": "a", "description": "file a"}
                ]
            }
        })
        .to_string();
        let transport = MockTransport::new(vec![res_response]);
        let mut client = McpClient::new(transport, "test");
        let resources = client.list_resources().await.unwrap();
        assert_eq!(resources.len(), 1);
        assert_eq!(resources[0].uri, "file:///tmp/a");
    }

    #[tokio::test]
    async fn test_read_resource() {
        let read_response = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "contents": [
                    {"type": "text", "uri": "file:///tmp/a", "text": "content"}
                ]
            }
        })
        .to_string();
        let transport = MockTransport::new(vec![read_response]);
        let mut client = McpClient::new(transport, "test");
        let contents = client.read_resource("file:///tmp/a").await.unwrap();
        assert_eq!(contents.len(), 1);
        assert_eq!(contents[0].uri, "file:///tmp/a");
        assert_eq!(contents[0].text, Some("content".to_string()));
    }

    #[tokio::test]
    async fn test_shutdown() {
        let transport = MockTransport::new(vec![]);
        let client = McpClient::new(transport, "test");
        client.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_error_response() {
        let error_response = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "error": {
                "code": -32601,
                "message": "Method not found"
            }
        })
        .to_string();
        let transport = MockTransport::new(vec![error_response]);
        let mut client = McpClient::new(transport, "test");
        let result = client.list_tools().await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Method not found"));
    }
}
