use omk::mcp::config::{McpConfig, McpServerConfig, TransportType};
use tempfile::TempDir;

#[tokio::test]
async fn test_config_backward_compatibility_stdio() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("mcp.json");
    let raw = r#"{"servers":{"fs":{"command":"npx","args":["-y","@modelcontextprotocol/server-filesystem"],"env":{}}}}"#;
    tokio::fs::write(&path, raw).await.unwrap();

    let config = McpConfig::load(&path).await.unwrap();
    assert_eq!(config.servers.len(), 1);
    let server = config.servers.get("fs").unwrap();
    match &server.transport {
        TransportType::Stdio { command, args, env } => {
            assert_eq!(command, "npx");
            assert_eq!(args, &["-y", "@modelcontextprotocol/server-filesystem"]);
            assert!(env.is_empty());
        }
        other => panic!("expected stdio transport, got {:?}", other),
    }
}

#[tokio::test]
async fn test_config_sse_http() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("mcp.json");
    let raw = r#"{"servers":{"context7":{"url":"https://context7.com/sse","headers":{"Authorization":"Bearer token"}}}}"#;
    tokio::fs::write(&path, raw).await.unwrap();

    let config = McpConfig::load(&path).await.unwrap();
    assert_eq!(config.servers.len(), 1);
    let server = config.servers.get("context7").unwrap();
    match &server.transport {
        TransportType::SseHttp { url, headers } => {
            assert_eq!(url, "https://context7.com/sse");
            assert_eq!(
                headers.get("Authorization"),
                Some(&"Bearer token".to_string())
            );
        }
        other => panic!("expected sse_http transport, got {:?}", other),
    }
}

#[tokio::test]
async fn test_config_mixed_transports() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("mcp.json");
    let raw = r#"{
        "servers":{
            "fs":{"command":"npx","args":["-y","@modelcontextprotocol/server-filesystem"]},
            "linear":{"url":"https://linear.dev/mcp","headers":{}}
        }
    }"#;
    tokio::fs::write(&path, raw).await.unwrap();

    let config = McpConfig::load(&path).await.unwrap();
    assert_eq!(config.servers.len(), 2);

    let fs = config.servers.get("fs").unwrap();
    assert!(matches!(fs.transport, TransportType::Stdio { .. }));

    let linear = config.servers.get("linear").unwrap();
    assert!(matches!(linear.transport, TransportType::SseHttp { .. }));
}

#[tokio::test]
async fn test_config_empty_is_default() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("mcp.json");
    tokio::fs::write(&path, "{}").await.unwrap();
    let config = McpConfig::load(&path).await.unwrap();
    assert!(config.servers.is_empty());
}

#[tokio::test]
async fn test_config_missing_is_default() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("nonexistent.json");
    let config = McpConfig::load(&path).await.unwrap();
    assert!(config.servers.is_empty());
}

#[test]
fn test_transport_type_default_is_stdio() {
    let default = TransportType::default();
    assert!(matches!(default, TransportType::Stdio { .. }));
}

#[test]
fn test_server_config_default() {
    let config = McpServerConfig::default();
    assert!(matches!(config.transport, TransportType::Stdio { .. }));
}
