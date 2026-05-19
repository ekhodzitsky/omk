#![allow(dead_code)]

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::AsyncWriteExt;
use tokio_stream::StreamExt;
use tokio_util::codec::{FramedRead, LinesCodec};
use tracing::{debug, error, info, warn};

use super::tools::{handle_tool_call, list_tools};
use crate::wire::protocol::scrub_secret_patterns;

/// Maximum length of a single inbound MCP JSON-RPC request, in bytes.
///
/// Bounds memory damage from a misbehaving / hostile MCP client that sends
/// a multi-GB payload without a newline. Mirrors the wire-side cap in
/// `crate::wire::client::MAX_WIRE_LINE_LENGTH`.
const MAX_MCP_LINE_LENGTH: usize = 16 * 1024 * 1024;

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Option<Value>,
}

#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
struct JsonRpcError {
    code: i32,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<Value>,
}

pub async fn run_mcp_server() -> Result<()> {
    info!("Starting OMK MCP server over stdio");

    let stdin = tokio::io::stdin();
    // Length-capped line reader. A non-cooperative MCP client that never
    // emits a newline can no longer drive the host to OOM.
    let mut reader = FramedRead::new(stdin, LinesCodec::new_with_max_length(MAX_MCP_LINE_LENGTH));
    let mut stdout = tokio::io::stdout();

    while let Some(line_result) = reader.next().await {
        let line = match line_result {
            Ok(line) => line,
            Err(e) => {
                // Most likely a max-line-length violation. Log and continue;
                // the framer drops the over-long line so the next message
                // boundary should still be discoverable.
                warn!(error = %e, "Failed to read MCP line (length cap or IO error); skipping");
                continue;
            }
        };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        debug!(line = %scrub_secret_patterns(line), "Received JSON-RPC request");

        let request: JsonRpcRequest = match serde_json::from_str(line) {
            Ok(req) => req,
            Err(e) => {
                let error_response = JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: None,
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32700,
                        message: format!("Parse error: {}", e),
                        data: None,
                    }),
                };
                if let Err(e) = send_response(&mut stdout, error_response).await {
                    error!(error = %e, "Failed to send parse error response");
                }
                continue;
            }
        };

        let is_notification = request.id.is_none();
        let response = handle_request(request).await;

        if !is_notification {
            if let Some(resp) = response {
                if let Err(e) = send_response(&mut stdout, resp).await {
                    error!(error = %e, "Failed to send response");
                }
            }
        }
    }

    info!("MCP server stdin closed, shutting down");
    Ok(())
}

async fn send_response(stdout: &mut tokio::io::Stdout, response: JsonRpcResponse) -> Result<()> {
    let json = serde_json::to_string(&response)?;
    stdout.write_all(json.as_bytes()).await?;
    stdout.write_all(b"\n").await?;
    stdout.flush().await?;
    Ok(())
}

async fn handle_request(request: JsonRpcRequest) -> Option<JsonRpcResponse> {
    let id = request.id.clone();

    match request.method.as_str() {
        "initialize" => Some(handle_initialize(id, request.params)),
        "notifications/initialized" => {
            debug!("Client initialized");
            None
        }
        "tools/list" => Some(handle_tools_list(id)),
        "tools/call" => match handle_tools_call(id.clone(), request.params).await {
            Ok(resp) => Some(resp),
            Err(e) => Some(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: None,
                error: Some(JsonRpcError {
                    code: -32603,
                    message: format!("Internal error: {}", e),
                    data: None,
                }),
            }),
        },
        _ => Some(JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(JsonRpcError {
                code: -32601,
                message: format!("Method not found: {}", request.method),
                data: None,
            }),
        }),
    }
}

fn handle_initialize(id: Option<Value>, _params: Option<Value>) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        id,
        result: Some(serde_json::json!({
            "protocolVersion": "2024-11-05",
            "serverInfo": {
                "name": "omk",
                "version": env!("CARGO_PKG_VERSION")
            },
            "capabilities": {
                "tools": {}
            }
        })),
        error: None,
    }
}

fn handle_tools_list(id: Option<Value>) -> JsonRpcResponse {
    let tools = list_tools();
    JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        id,
        result: Some(serde_json::json!({
            "tools": tools
        })),
        error: None,
    }
}

async fn handle_tools_call(id: Option<Value>, params: Option<Value>) -> Result<JsonRpcResponse> {
    let params = params.unwrap_or(Value::Null);
    let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
    let arguments = params.get("arguments").cloned().unwrap_or(Value::Null);

    match handle_tool_call(name, arguments).await {
        Ok(result) => Ok(JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(serde_json::json!({
                "content": [
                    {
                        "type": "text",
                        "text": serde_json::to_string_pretty(&result)?
                    }
                ]
            })),
            error: None,
        }),
        Err(e) => Ok(JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(serde_json::json!({
                "content": [
                    {
                        "type": "text",
                        "text": serde_json::to_string_pretty(&e).unwrap_or_else(|_| e.to_string())
                    }
                ],
                "isError": true,
                "error": e
            })),
            error: None,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handle_initialize_returns_protocol_version() {
        let resp = handle_initialize(Some(serde_json::json!("init-1")), None);
        assert_eq!(resp.jsonrpc, "2.0");
        assert_eq!(resp.id, Some(serde_json::json!("init-1")));
        assert!(resp.error.is_none());

        let result = resp.result.unwrap();
        assert_eq!(result["protocolVersion"], "2024-11-05");
        assert_eq!(result["serverInfo"]["name"], "omk");
        assert!(result["capabilities"]["tools"].is_object());
    }

    #[test]
    fn handle_tools_list_returns_tools_array() {
        let resp = handle_tools_list(Some(serde_json::json!("list-1")));
        assert_eq!(resp.jsonrpc, "2.0");
        assert_eq!(resp.id, Some(serde_json::json!("list-1")));
        assert!(resp.error.is_none());

        let result = resp.result.unwrap();
        let tools = result["tools"].as_array().expect("tools array");
        assert!(!tools.is_empty());
        assert!(tools.iter().all(|t| t["name"].is_string()));
    }

    #[tokio::test]
    async fn handle_request_initialize() {
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!("r1")),
            method: "initialize".to_string(),
            params: None,
        };
        let resp = handle_request(req).await.unwrap();
        assert!(resp.result.is_some());
        assert!(resp.error.is_none());
    }

    #[tokio::test]
    async fn handle_request_notifications_initialized_is_silent() {
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: None,
            method: "notifications/initialized".to_string(),
            params: None,
        };
        let resp = handle_request(req).await;
        assert!(resp.is_none());
    }

    #[tokio::test]
    async fn handle_request_tools_list() {
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!("r2")),
            method: "tools/list".to_string(),
            params: None,
        };
        let resp = handle_request(req).await.unwrap();
        assert!(resp.result.is_some());
        assert!(resp.error.is_none());
    }

    #[tokio::test]
    async fn handle_request_unknown_method() {
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!("r3")),
            method: "unknown/method".to_string(),
            params: None,
        };
        let resp = handle_request(req).await.unwrap();
        assert!(resp.result.is_none());
        assert!(resp.error.is_some());
        let err = resp.error.unwrap();
        assert_eq!(err.code, -32601);
        assert!(err.message.contains("Method not found"));
    }

    #[tokio::test]
    async fn handle_tools_call_unknown_tool_returns_error_response() {
        let resp = handle_tools_call(
            Some(serde_json::json!("tc1")),
            Some(serde_json::json!({"name": "unknown_tool", "arguments": {}})),
        )
        .await
        .unwrap();

        assert!(resp.result.is_some());
        assert!(resp.error.is_none());
        let result = resp.result.unwrap();
        assert_eq!(result["isError"], true);
    }

    #[tokio::test]
    async fn handle_tools_call_missing_name_defaults_to_empty() {
        let resp = handle_tools_call(
            Some(serde_json::json!("tc2")),
            Some(serde_json::json!({"arguments": {}})),
        )
        .await
        .unwrap();

        assert!(resp.result.is_some());
        let result = resp.result.unwrap();
        assert_eq!(result["isError"], true);
    }
}
