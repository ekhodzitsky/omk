#![allow(dead_code)]

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
use tracing::{debug, error, info};

use super::tools::{handle_tool_call, list_tools};

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
    let mut reader = tokio::io::BufReader::new(stdin).lines();
    let mut stdout = tokio::io::stdout();

    while let Some(line) = reader.next_line().await? {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        debug!(line = %line, "Received JSON-RPC request");

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
