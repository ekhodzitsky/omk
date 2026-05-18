use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::time::Duration;

use anyhow::{Context, Result};
use moka::future::Cache;
use serde_json::Value;

use super::types::CallToolResult;

/// Abstracts over anything that can call an MCP tool.
pub trait McpToolCaller {
    /// Returns the server name for cache key generation.
    fn server_name(&self) -> &str;

    /// Calls an MCP tool by name with JSON arguments.
    fn call_tool(
        &mut self,
        name: &str,
        arguments: Value,
    ) -> impl std::future::Future<Output = Result<CallToolResult>> + Send;
}

impl<T: super::transport_trait::McpTransport> McpToolCaller for super::core::McpClient<T> {
    fn server_name(&self) -> &str {
        self.server_name()
    }

    fn call_tool(
        &mut self,
        name: &str,
        arguments: Value,
    ) -> impl std::future::Future<Output = Result<CallToolResult>> + Send {
        self.call_tool(name, arguments)
    }
}

/// A caching wrapper around an [`McpToolCaller`].
///
/// Caches successful `call_tool` results keyed by a hash of
/// `(server_name, tool_name, serialized_args)`.
pub struct CachedMcpClient<C: McpToolCaller> {
    inner: C,
    cache: Cache<String, CallToolResult>,
}

impl<C: McpToolCaller> std::fmt::Debug for CachedMcpClient<C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CachedMcpClient")
            .field("cache", &self.cache)
            .finish_non_exhaustive()
    }
}

impl<C: McpToolCaller> CachedMcpClient<C> {
    /// Creates a new cached client with the default TTL (5 minutes) and max capacity (1000).
    pub fn new(inner: C) -> Self {
        Self::with_config(inner, 1000, Duration::from_secs(300))
    }

    /// Creates a new cached client with custom capacity and TTL.
    pub fn with_config(inner: C, max_capacity: u64, ttl: Duration) -> Self {
        let cache = Cache::builder()
            .max_capacity(max_capacity)
            .time_to_live(ttl)
            .build();
        Self { inner, cache }
    }

    /// Calls a tool, returning a cached result if available.
    pub async fn call_tool(&mut self, name: &str, arguments: Value) -> Result<CallToolResult> {
        let key = cache_key(self.inner.server_name(), name, &arguments)
            .context("failed to build cache key")?;

        if let Some(cached) = self.cache.get(&key).await {
            return Ok(cached);
        }

        let result = self.inner.call_tool(name, arguments).await?;
        self.cache.insert(key, result.clone()).await;
        Ok(result)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::client::types::ToolContent;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    struct MockToolCaller {
        server_name: String,
        call_count: Arc<AtomicUsize>,
        result: CallToolResult,
    }

    impl MockToolCaller {
        fn new(server_name: &str, result: CallToolResult) -> Self {
            Self {
                server_name: server_name.to_string(),
                call_count: Arc::new(AtomicUsize::new(0)),
                result,
            }
        }
    }

    impl McpToolCaller for MockToolCaller {
        fn server_name(&self) -> &str {
            &self.server_name
        }

        fn call_tool(
            &mut self,
            _name: &str,
            _arguments: Value,
        ) -> impl std::future::Future<Output = Result<CallToolResult>> + Send {
            let count = self.call_count.clone();
            let result = self.result.clone();
            async move {
                count.fetch_add(1, Ordering::SeqCst);
                Ok(result)
            }
        }
    }

    #[tokio::test]
    async fn test_cache_hit_reduces_calls() {
        let result = CallToolResult {
            content: vec![ToolContent::Text {
                text: "hello".to_string(),
            }],
            is_error: Some(false),
        };
        let mock = MockToolCaller::new("test-server", result);
        let count = mock.call_count.clone();
        let mut cached = CachedMcpClient::new(mock);

        let args = serde_json::json!({"key": "value"});
        let r1 = cached.call_tool("tool-a", args.clone()).await.unwrap();
        let r2 = cached.call_tool("tool-a", args.clone()).await.unwrap();

        assert_eq!(r1, r2);
        assert_eq!(
            count.load(Ordering::SeqCst),
            1,
            "inner client should only be called once"
        );
    }

    #[tokio::test]
    async fn test_cache_miss_different_args() {
        let result = CallToolResult {
            content: vec![ToolContent::Text {
                text: "hello".to_string(),
            }],
            is_error: Some(false),
        };
        let mock = MockToolCaller::new("test-server", result);
        let count = mock.call_count.clone();
        let mut cached = CachedMcpClient::new(mock);

        let _ = cached
            .call_tool("tool-a", serde_json::json!({"key": "a"}))
            .await
            .unwrap();
        let _ = cached
            .call_tool("tool-a", serde_json::json!({"key": "b"}))
            .await
            .unwrap();

        assert_eq!(count.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn test_cache_miss_different_tool() {
        let result = CallToolResult {
            content: vec![ToolContent::Text {
                text: "hello".to_string(),
            }],
            is_error: Some(false),
        };
        let mock = MockToolCaller::new("test-server", result);
        let count = mock.call_count.clone();
        let mut cached = CachedMcpClient::new(mock);

        let _ = cached
            .call_tool("tool-a", serde_json::json!({}))
            .await
            .unwrap();
        let _ = cached
            .call_tool("tool-b", serde_json::json!({}))
            .await
            .unwrap();

        assert_eq!(count.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn test_cache_key_includes_server_name() {
        let result = CallToolResult {
            content: vec![ToolContent::Text {
                text: "hello".to_string(),
            }],
            is_error: Some(false),
        };
        let mock = MockToolCaller::new("server-a", result);
        let count = mock.call_count.clone();
        let mut cached = CachedMcpClient::new(mock);

        let _ = cached
            .call_tool("tool-a", serde_json::json!({}))
            .await
            .unwrap();
        let _ = cached
            .call_tool("tool-a", serde_json::json!({}))
            .await
            .unwrap();

        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_with_custom_ttl() {
        let result = CallToolResult {
            content: vec![ToolContent::Text {
                text: "hello".to_string(),
            }],
            is_error: Some(false),
        };
        let mock = MockToolCaller::new("test-server", result);
        let count = mock.call_count.clone();
        let mut cached = CachedMcpClient::with_config(mock, 100, Duration::from_secs(60));

        let _ = cached
            .call_tool("tool-a", serde_json::json!({}))
            .await
            .unwrap();
        let _ = cached
            .call_tool("tool-a", serde_json::json!({}))
            .await
            .unwrap();

        assert_eq!(count.load(Ordering::SeqCst), 1);
    }
}
