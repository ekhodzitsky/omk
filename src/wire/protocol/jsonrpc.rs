use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JsonRpcRequest<Params> {
    pub jsonrpc: String,
    pub method: String,
    pub id: String,
    pub params: Params,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JsonRpcNotification<Params> {
    pub jsonrpc: String,
    pub method: String,
    pub params: Params,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JsonRpcSuccessResponse<Result> {
    pub jsonrpc: String,
    pub id: String,
    pub result: Result,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JsonRpcErrorResponse {
    pub jsonrpc: String,
    pub id: String,
    pub error: JsonRpcError,
}

/// Catch-all wire message used for low-level parsing before dispatch.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WireMessage {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::protocol::InitializeParams;
    use serde_json::json;

    #[test]
    fn test_jsonrpc_request_roundtrip() {
        use crate::wire::protocol::{ClientCapabilities, ClientInfo};
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "initialize".to_string(),
            id: "init-1".to_string(),
            params: InitializeParams {
                protocol_version: "1.9".to_string(),
                client: Some(ClientInfo {
                    name: "test-client".to_string(),
                    version: Some("1.0.0".to_string()),
                }),
                external_tools: None,
                capabilities: Some(ClientCapabilities {
                    supports_question: Some(true),
                    supports_plan_mode: Some(false),
                }),
                hooks: None,
            },
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"jsonrpc\":\"2.0\""));
        assert!(json.contains("\"method\":\"initialize\""));
        assert!(json.contains("\"protocol_version\":\"1.9\""));
        assert!(json.contains("\"supports_question\":true"));
        assert!(json.contains("\"supports_plan_mode\":false"));
        let de: JsonRpcRequest<InitializeParams> = serde_json::from_str(&json).unwrap();
        assert_eq!(de, req);
    }

    #[test]
    fn test_jsonrpc_success_response_roundtrip() {
        use crate::wire::protocol::InitializeResult;
        let res = JsonRpcSuccessResponse {
            jsonrpc: "2.0".to_string(),
            id: "init-1".to_string(),
            result: InitializeResult {
                protocol_version: "1.9".to_string(),
                server: None,
                slash_commands: None,
                external_tools: None,
                capabilities: None,
                hooks: None,
            },
        };
        let json = serde_json::to_string(&res).unwrap();
        assert!(json.contains("\"result\""));
        assert!(json.contains("\"protocol_version\":\"1.9\""));
        let de: JsonRpcSuccessResponse<InitializeResult> = serde_json::from_str(&json).unwrap();
        assert_eq!(de, res);
    }

    #[test]
    fn test_jsonrpc_error_response_roundtrip() {
        let res = JsonRpcErrorResponse {
            jsonrpc: "2.0".to_string(),
            id: "req-1".to_string(),
            error: JsonRpcError {
                code: -32600,
                message: "Invalid Request".to_string(),
                data: Some(json!({"detail": "missing method"})),
            },
        };
        let json = serde_json::to_string(&res).unwrap();
        assert!(json.contains("\"code\":-32600"));
        assert!(json.contains("\"message\":\"Invalid Request\""));
        let de: JsonRpcErrorResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(de, res);
    }

    #[test]
    fn test_wire_message_roundtrip() {
        let msg = WireMessage {
            jsonrpc: "2.0".to_string(),
            id: Some("1".to_string()),
            method: Some("prompt".to_string()),
            params: Some(json!({"user_input": "hello"})),
            result: None,
            error: None,
        };
        let json = serde_json::to_string(&msg).unwrap();
        let de: WireMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(de, msg);
    }

    #[test]
    fn jsonrpc_request_initialize_roundtrip() {
        let cases = [
            ("init-1", "initialize", "1.9"),
            ("prompt-2", "prompt", "1.0"),
            ("request_3", "set_plan_mode", "1.7"),
        ];

        for (id, method, version) in cases {
            let req = JsonRpcRequest {
                jsonrpc: "2.0".to_string(),
                method: method.to_string(),
                id: id.to_string(),
                params: InitializeParams {
                    protocol_version: version.to_string(),
                    client: None,
                    external_tools: None,
                    capabilities: None,
                    hooks: None,
                },
            };
            let json = serde_json::to_string(&req).unwrap();
            let restored: JsonRpcRequest<InitializeParams> = serde_json::from_str(&json).unwrap();
            assert_eq!(restored, req);
        }
    }
}
