mod request;
mod response;
mod types;

pub use types::McpClient;

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::mcp::client::transport_trait::McpTransport;

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
        ) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + '_>> {
            self.sent.lock().unwrap().push(message);
            Box::pin(async move { Ok(()) })
        }

        fn recv(&mut self) -> Pin<Box<dyn Future<Output = anyhow::Result<Option<String>>> + Send + '_>> {
            let responses = self.responses.clone();
            Box::pin(async move { Ok(responses.lock().unwrap().pop_front()) })
        }

        fn close(&mut self) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + '_>> {
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
