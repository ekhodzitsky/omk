#![allow(clippy::enum_variant_names)]

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Wire protocol version observed from `kimi info` on Kimi Code CLI 1.41.0.
pub const KIMI_WIRE_PROTOCOL_VERSION: &str = "1.9";

// ============================================================================
// JSON-RPC 2.0 Base Types
// ============================================================================

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

// ============================================================================
// Initialize
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InitializeParams {
    pub protocol_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client: Option<ClientInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_tools: Option<Vec<Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<ClientCapabilities>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hooks: Option<Vec<WireHookSubscription>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClientInfo {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClientCapabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supports_question: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supports_plan_mode: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WireHookSubscription {
    pub id: String,
    pub event: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matcher: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InitializeResult {
    pub protocol_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slash_commands: Option<Vec<Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_tools: Option<Vec<Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hooks: Option<Vec<Value>>,
}

// ============================================================================
// Prompt
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PromptParams {
    pub user_input: UserInput,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum UserInput {
    Text(String),
    Parts(Vec<ContentPart>),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PromptResult {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub steps: Option<PromptSteps>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum PromptSteps {
    Count(u64),
    LegacyTrace(Vec<Value>),
}

// ============================================================================
// Replay
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ReplayParams {}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReplayResult {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub events: Option<Vec<Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requests: Option<Vec<Value>>,
}

// ============================================================================
// Steer
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SteerParams {
    pub user_input: UserInput,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SteerResult {
    pub status: String,
}

// ============================================================================
// SetPlanMode
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SetPlanModeParams {
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SetPlanModeResult {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan_mode: Option<bool>,
}

// ============================================================================
// Cancel
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct CancelParams {}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct CancelResult {}

// ============================================================================
// ContentPart
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentPart {
    Text(TextPart),
    Think(ThinkPart),
    #[serde(rename = "image_url")]
    ImageUrl(ImageUrlPart),
    #[serde(rename = "audio_url")]
    AudioUrl(AudioUrlPart),
    #[serde(rename = "video_url")]
    VideoUrl(VideoUrlPart),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TextPart {
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ThinkPart {
    pub think: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encrypted: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ImageUrlPart {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AudioUrlPart {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VideoUrlPart {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

// ============================================================================
// DisplayBlock
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DisplayBlock {
    Brief(BriefDisplayBlock),
    Diff(DiffDisplayBlock),
    Todo(TodoDisplayBlock),
    Shell(ShellDisplayBlock),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BriefDisplayBlock {
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DiffDisplayBlock {
    pub path: String,
    pub old_text: String,
    pub new_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TodoDisplayBlock {
    pub items: Vec<TodoDisplayItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TodoDisplayItem {
    pub title: String,
    pub status: TodoStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TodoStatus {
    Pending,
    InProgress,
    Done,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ShellDisplayBlock {
    pub language: String,
    pub command: String,
}

// ============================================================================
// Event Types
// ============================================================================

/// Raw wire params for the `event` notification.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EventParams {
    #[serde(rename = "type")]
    pub event_type: String,
    pub payload: Value,
}

/// Typed event union for convenience.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    TurnBegin {
        user_input: String,
    },
    TurnEnd,
    StepBegin {
        n: u32,
    },
    StepInterrupted,
    CompactionBegin,
    CompactionEnd,
    StatusUpdate {
        #[serde(skip_serializing_if = "Option::is_none")]
        context_usage: Option<f64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        context_tokens: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        max_context_tokens: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        token_usage: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        message_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        plan_mode: Option<bool>,
    },
    #[serde(rename = "function")]
    ToolCall {
        id: String,
        function: ToolCallFunction,
        #[serde(skip_serializing_if = "Option::is_none")]
        extras: Option<Value>,
    },
    ToolCallPart {
        #[serde(skip_serializing_if = "Option::is_none")]
        arguments_part: Option<String>,
    },
    ToolResult {
        tool_call_id: String,
        return_value: ToolReturnValue,
    },
    ApprovalResponse {
        request_id: String,
        response: ApprovalResponseType,
        #[serde(skip_serializing_if = "Option::is_none")]
        feedback: Option<String>,
    },
    SubagentEvent {
        #[serde(skip_serializing_if = "Option::is_none")]
        parent_tool_call_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        agent_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        subagent_type: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        event: Option<Box<Event>>,
    },
    SteerInput {
        user_input: String,
    },
    PlanDisplay {
        content: String,
        file_path: String,
    },
    HookTriggered {
        event: String,
        target: String,
        hook_count: u32,
    },
    HookResolved {
        event: String,
        target: String,
        action: HookAction,
        reason: String,
        duration_ms: u64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolCallFunction {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolReturnValue {
    pub is_error: bool,
    pub output: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display: Option<Vec<DisplayBlock>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extras: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalResponseType {
    Approve,
    #[serde(rename = "approve_for_session")]
    ApproveForSession,
    Reject,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum HookAction {
    Allow,
    Block,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StatusUpdate {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_usage: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_context_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_usage: Option<TokenUsage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan_mode: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TokenUsage {
    pub input_other: u64,
    pub output: u64,
    pub input_cache_read: u64,
    pub input_cache_creation: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolCall {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub id: String,
    pub function: ToolCallFunction,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extras: Option<Value>,
}

// ============================================================================
// Request Types
// ============================================================================

/// Raw wire params for the `request` method.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RequestParams {
    #[serde(rename = "type")]
    pub request_type: String,
    pub payload: Value,
}

/// Typed request union for convenience.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Request {
    #[serde(rename = "ApprovalRequest")]
    ApprovalRequest(ApprovalRequest),
    #[serde(rename = "ToolCallRequest")]
    ToolCallRequest(ToolCallRequest),
    #[serde(rename = "QuestionRequest")]
    QuestionRequest(QuestionRequest),
    #[serde(rename = "HookRequest")]
    HookRequest(HookRequest),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ApprovalRequest {
    pub id: String,
    pub tool_call_id: String,
    pub sender: String,
    pub action: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display: Option<Vec<DisplayBlock>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subagent_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolCallRequest {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QuestionRequest {
    pub id: String,
    pub tool_call_id: String,
    pub questions: Vec<Question>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Question {
    pub question: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub header: Option<String>,
    pub options: Vec<QuestionOption>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub multi_select: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QuestionOption {
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HookRequest {
    pub id: String,
    pub subscription_id: String,
    pub event: String,
    pub target: String,
    pub input_data: Value,
}

// ============================================================================
// Conversions
// ============================================================================

impl Event {
    /// Convert a typed event into raw wire params.
    pub fn to_params(&self) -> Result<EventParams, serde_json::Error> {
        let mut value = serde_json::to_value(self)?;
        let event_type = if let Value::Object(ref mut map) = value {
            map.remove("type")
                .and_then(|v| v.as_str().map(String::from))
                .unwrap_or_default()
        } else {
            String::new()
        };
        Ok(EventParams {
            event_type,
            payload: value,
        })
    }
}

impl EventParams {
    /// Convert raw wire params into a typed event.
    pub fn to_event(&self) -> Result<Event, serde_json::Error> {
        let mut value = self.payload.clone();
        if let Value::Object(ref mut map) = value {
            map.insert("type".to_string(), Value::String(self.event_type.clone()));
        }
        serde_json::from_value(value)
    }
}

impl Request {
    /// Convert a typed request into raw wire params.
    pub fn to_params(&self) -> Result<RequestParams, serde_json::Error> {
        let mut value = serde_json::to_value(self)?;
        let request_type = if let Value::Object(ref mut map) = value {
            map.remove("type")
                .and_then(|v| v.as_str().map(String::from))
                .unwrap_or_default()
        } else {
            String::new()
        };
        Ok(RequestParams {
            request_type,
            payload: value,
        })
    }

    /// Conservative default response for requests emitted by Kimi during a turn.
    ///
    /// OMK currently does not register external tools or interactive question
    /// capabilities with Kimi, but real wire sessions can still surface these
    /// request types. Returning a structured, typed response keeps the turn
    /// alive and records the mismatch without aborting the JSON-RPC session.
    pub fn default_response(&self) -> Value {
        match self {
            Request::ApprovalRequest(request) => serde_json::json!({
                "request_id": request.id,
                "response": ApprovalResponseType::ApproveForSession,
                "feedback": "OMK auto-approved this non-interactive worker request."
            }),
            Request::ToolCallRequest(request) => serde_json::json!({
                "tool_call_id": request.id,
                "return_value": ToolReturnValue {
                    is_error: true,
                    output: String::new(),
                    message: format!(
                        "OMK did not register external tool '{}' for this worker.",
                        request.name
                    ),
                    display: Some(vec![DisplayBlock::Brief(BriefDisplayBlock {
                        text: "External tool unavailable in OMK wire worker.".to_string(),
                    })]),
                    extras: None,
                }
            }),
            Request::QuestionRequest(request) => {
                let answers: Vec<Value> = request
                    .questions
                    .iter()
                    .map(|question| {
                        question
                            .options
                            .first()
                            .map(|option| Value::String(option.label.clone()))
                            .unwrap_or(Value::Null)
                    })
                    .collect();
                serde_json::json!({
                    "request_id": request.id,
                    "answers": answers,
                    "message": "OMK selected default answers because workers run non-interactively."
                })
            }
            Request::HookRequest(request) => serde_json::json!({
                "request_id": request.id,
                "action": HookAction::Allow,
                "reason": format!(
                    "No OMK hook policy is configured for '{}' on '{}'.",
                    request.event, request.target
                )
            }),
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Request::ApprovalRequest(_) => "ApprovalRequest",
            Request::ToolCallRequest(_) => "ToolCallRequest",
            Request::QuestionRequest(_) => "QuestionRequest",
            Request::HookRequest(_) => "HookRequest",
        }
    }
}

impl RequestParams {
    /// Convert raw wire params into a typed request.
    pub fn to_request(&self) -> Result<Request, serde_json::Error> {
        let mut value = self.payload.clone();
        if let Value::Object(ref mut map) = value {
            map.insert("type".to_string(), Value::String(self.request_type.clone()));
        }
        serde_json::from_value(value)
    }
}

const REDACTED_SECRET: &str = "[REDACTED]";

/// Redact sensitive fields in wire JSON payloads while preserving structure.
///
/// This is used before writing durable wire logs/evidence.
pub fn redact_wire_secrets(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut redacted = serde_json::Map::with_capacity(map.len());
            for (key, entry) in map {
                if is_sensitive_wire_key(key) {
                    redacted.insert(key.clone(), Value::String(REDACTED_SECRET.to_string()));
                } else {
                    redacted.insert(key.clone(), redact_wire_secrets(entry));
                }
            }
            Value::Object(redacted)
        }
        Value::Array(items) => Value::Array(items.iter().map(redact_wire_secrets).collect()),
        _ => value.clone(),
    }
}

fn is_sensitive_wire_key(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    matches!(
        lower.as_str(),
        "api_key" | "apikey" | "token" | "authorization" | "password" | "secret"
    ) || lower.ends_with("_token")
        || lower.ends_with("-token")
        || lower.ends_with("_secret")
        || lower.ends_with("-secret")
        || lower.contains("authorization")
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_jsonrpc_request_roundtrip() {
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
    fn test_prompt_params_text() {
        let params = PromptParams {
            user_input: UserInput::Text("hello world".to_string()),
        };
        let json = serde_json::to_value(&params).unwrap();
        assert_eq!(json["user_input"], "hello world");
        let de: PromptParams = serde_json::from_value(json).unwrap();
        assert_eq!(de, params);
    }

    #[test]
    fn test_prompt_params_parts() {
        let params = PromptParams {
            user_input: UserInput::Parts(vec![
                ContentPart::Text(TextPart {
                    text: "hello".to_string(),
                }),
                ContentPart::Think(ThinkPart {
                    think: "thinking...".to_string(),
                    encrypted: Some(true),
                }),
            ]),
        };
        let json = serde_json::to_value(&params).unwrap();
        let parts = json["user_input"].as_array().unwrap();
        assert_eq!(parts[0]["type"], "text");
        assert_eq!(parts[0]["text"], "hello");
        assert_eq!(parts[1]["type"], "think");
        assert_eq!(parts[1]["think"], "thinking...");
        assert_eq!(parts[1]["encrypted"], true);
        let de: PromptParams = serde_json::from_value(json).unwrap();
        assert_eq!(de, params);
    }

    #[test]
    fn test_content_part_image_url() {
        let part = ContentPart::ImageUrl(ImageUrlPart {
            url: Some("https://example.com/img.png".to_string()),
        });
        let json = serde_json::to_value(&part).unwrap();
        assert_eq!(json["type"], "image_url");
        assert_eq!(json["url"], "https://example.com/img.png");
        let de: ContentPart = serde_json::from_value(json).unwrap();
        assert_eq!(de, part);
    }

    #[test]
    fn test_content_part_audio_url() {
        let part = ContentPart::AudioUrl(AudioUrlPart {
            url: Some("https://example.com/audio.mp3".to_string()),
        });
        let json = serde_json::to_value(&part).unwrap();
        assert_eq!(json["type"], "audio_url");
        let de: ContentPart = serde_json::from_value(json).unwrap();
        assert_eq!(de, part);
    }

    #[test]
    fn test_content_part_video_url() {
        let part = ContentPart::VideoUrl(VideoUrlPart {
            url: Some("https://example.com/video.mp4".to_string()),
        });
        let json = serde_json::to_value(&part).unwrap();
        assert_eq!(json["type"], "video_url");
        let de: ContentPart = serde_json::from_value(json).unwrap();
        assert_eq!(de, part);
    }

    #[test]
    fn test_display_block_brief() {
        let block = DisplayBlock::Brief(BriefDisplayBlock {
            text: "summary".to_string(),
        });
        let json = serde_json::to_value(&block).unwrap();
        assert_eq!(json["type"], "brief");
        assert_eq!(json["text"], "summary");
        let de: DisplayBlock = serde_json::from_value(json).unwrap();
        assert_eq!(de, block);
    }

    #[test]
    fn test_display_block_diff() {
        let block = DisplayBlock::Diff(DiffDisplayBlock {
            path: "/tmp/test.txt".to_string(),
            old_text: "old".to_string(),
            new_text: "new".to_string(),
        });
        let json = serde_json::to_value(&block).unwrap();
        assert_eq!(json["type"], "diff");
        assert_eq!(json["path"], "/tmp/test.txt");
        let de: DisplayBlock = serde_json::from_value(json).unwrap();
        assert_eq!(de, block);
    }

    #[test]
    fn test_display_block_todo() {
        let block = DisplayBlock::Todo(TodoDisplayBlock {
            items: vec![
                TodoDisplayItem {
                    title: "task 1".to_string(),
                    status: TodoStatus::Pending,
                },
                TodoDisplayItem {
                    title: "task 2".to_string(),
                    status: TodoStatus::Done,
                },
            ],
        });
        let json = serde_json::to_value(&block).unwrap();
        assert_eq!(json["type"], "todo");
        assert_eq!(json["items"][0]["status"], "pending");
        assert_eq!(json["items"][1]["status"], "done");
        let de: DisplayBlock = serde_json::from_value(json).unwrap();
        assert_eq!(de, block);
    }

    #[test]
    fn test_display_block_shell() {
        let block = DisplayBlock::Shell(ShellDisplayBlock {
            language: "bash".to_string(),
            command: "echo hello".to_string(),
        });
        let json = serde_json::to_value(&block).unwrap();
        assert_eq!(json["type"], "shell");
        assert_eq!(json["language"], "bash");
        assert_eq!(json["command"], "echo hello");
        let de: DisplayBlock = serde_json::from_value(json).unwrap();
        assert_eq!(de, block);
    }

    #[test]
    fn test_event_turn_begin_roundtrip() {
        let event = Event::TurnBegin {
            user_input: "hello".to_string(),
        };
        let params = event.to_params().unwrap();
        assert_eq!(params.event_type, "turn_begin");
        assert_eq!(params.payload["user_input"], "hello");
        let back = params.to_event().unwrap();
        assert_eq!(back, event);
    }

    #[test]
    fn test_event_tool_call_roundtrip() {
        let event = Event::ToolCall {
            id: "call_1".to_string(),
            function: ToolCallFunction {
                name: "read_file".to_string(),
                arguments: Some(json!({"path": "/tmp/test"}).to_string()),
            },
            extras: None,
        };
        let params = event.to_params().unwrap();
        assert_eq!(params.event_type, "function");
        assert_eq!(params.payload["id"], "call_1");
        assert_eq!(params.payload["function"]["name"], "read_file");
        let back = params.to_event().unwrap();
        assert_eq!(back, event);
    }

    #[test]
    fn test_event_status_update_roundtrip() {
        let event = Event::StatusUpdate {
            context_usage: Some(0.75),
            context_tokens: Some(1024),
            max_context_tokens: Some(4096),
            token_usage: Some(512),
            message_id: Some("msg-1".to_string()),
            plan_mode: Some(true),
        };
        let params = event.to_params().unwrap();
        assert_eq!(params.event_type, "status_update");
        let back = params.to_event().unwrap();
        assert_eq!(back, event);
    }

    #[test]
    fn test_event_subagent_event_roundtrip() {
        let event = Event::SubagentEvent {
            parent_tool_call_id: Some("parent_1".to_string()),
            agent_id: Some("agent_1".to_string()),
            subagent_type: Some("explore".to_string()),
            event: Some(Box::new(Event::TurnEnd)),
        };
        let params = event.to_params().unwrap();
        assert_eq!(params.event_type, "subagent_event");
        let back = params.to_event().unwrap();
        assert_eq!(back, event);
    }

    #[test]
    fn test_event_hook_resolved_roundtrip() {
        let event = Event::HookResolved {
            event: "file_write".to_string(),
            target: "/tmp/test".to_string(),
            action: HookAction::Allow,
            reason: "allowed by user".to_string(),
            duration_ms: 42,
        };
        let params = event.to_params().unwrap();
        assert_eq!(params.event_type, "hook_resolved");
        assert_eq!(params.payload["action"], "allow");
        let back = params.to_event().unwrap();
        assert_eq!(back, event);
    }

    #[test]
    fn test_request_approval_request_roundtrip() {
        let request = Request::ApprovalRequest(ApprovalRequest {
            id: "approval_1".to_string(),
            tool_call_id: "call_1".to_string(),
            sender: "agent".to_string(),
            action: "write_file".to_string(),
            description: "write to /tmp/test".to_string(),
            display: Some(vec![DisplayBlock::Brief(BriefDisplayBlock {
                text: "writing file".to_string(),
            })]),
            source_kind: None,
            source_id: None,
            agent_id: None,
            subagent_type: None,
            source_description: None,
        });
        let params = request.to_params().unwrap();
        assert_eq!(params.request_type, "ApprovalRequest");
        assert_eq!(params.payload["id"], "approval_1");
        let back = params.to_request().unwrap();
        assert_eq!(back, request);
    }

    #[test]
    fn test_request_tool_call_request_roundtrip() {
        let request = Request::ToolCallRequest(ToolCallRequest {
            id: "tool_1".to_string(),
            name: "read_file".to_string(),
            arguments: Some("{\"path\":\"/tmp/test\"}".to_string()),
        });
        let params = request.to_params().unwrap();
        assert_eq!(params.request_type, "ToolCallRequest");
        let back = params.to_request().unwrap();
        assert_eq!(back, request);
    }

    #[test]
    fn test_request_question_request_roundtrip() {
        let request = Request::QuestionRequest(QuestionRequest {
            id: "q_1".to_string(),
            tool_call_id: "call_1".to_string(),
            questions: vec![Question {
                question: "Continue?".to_string(),
                header: Some("Confirmation".to_string()),
                options: vec![
                    QuestionOption {
                        label: "Yes".to_string(),
                        description: Some("Proceed".to_string()),
                    },
                    QuestionOption {
                        label: "No".to_string(),
                        description: None,
                    },
                ],
                multi_select: Some(false),
            }],
        });
        let params = request.to_params().unwrap();
        assert_eq!(params.request_type, "QuestionRequest");
        let back = params.to_request().unwrap();
        assert_eq!(back, request);
    }

    #[test]
    fn test_request_hook_request_roundtrip() {
        let request = Request::HookRequest(HookRequest {
            id: "hook_1".to_string(),
            subscription_id: "sub_1".to_string(),
            event: "file_write".to_string(),
            target: "/tmp/test".to_string(),
            input_data: json!({"content": "hello"}),
        });
        let params = request.to_params().unwrap();
        assert_eq!(params.request_type, "HookRequest");
        let back = params.to_request().unwrap();
        assert_eq!(back, request);
    }

    #[test]
    fn test_default_request_responses_are_structured() {
        let approval = Request::ApprovalRequest(ApprovalRequest {
            id: "approval_1".to_string(),
            tool_call_id: "call_1".to_string(),
            sender: "agent".to_string(),
            action: "write_file".to_string(),
            description: "write to /tmp/test".to_string(),
            display: None,
            source_kind: None,
            source_id: None,
            agent_id: None,
            subagent_type: None,
            source_description: None,
        })
        .default_response();
        assert_eq!(approval["request_id"], "approval_1");
        assert_eq!(approval["response"], "approve_for_session");

        let tool = Request::ToolCallRequest(ToolCallRequest {
            id: "tool_1".to_string(),
            name: "read_file".to_string(),
            arguments: None,
        })
        .default_response();
        assert_eq!(tool["tool_call_id"], "tool_1");
        assert_eq!(tool["return_value"]["is_error"], true);
        assert!(tool["return_value"]["message"]
            .as_str()
            .unwrap()
            .contains("read_file"));

        let question = Request::QuestionRequest(QuestionRequest {
            id: "question_1".to_string(),
            tool_call_id: "call_1".to_string(),
            questions: vec![Question {
                question: "Continue?".to_string(),
                header: None,
                options: vec![QuestionOption {
                    label: "Yes".to_string(),
                    description: None,
                }],
                multi_select: None,
            }],
        })
        .default_response();
        assert_eq!(question["request_id"], "question_1");
        assert_eq!(question["answers"][0], "Yes");

        let hook = Request::HookRequest(HookRequest {
            id: "hook_1".to_string(),
            subscription_id: "sub_1".to_string(),
            event: "file_write".to_string(),
            target: "/tmp/test".to_string(),
            input_data: json!({}),
        })
        .default_response();
        assert_eq!(hook["request_id"], "hook_1");
        assert_eq!(hook["action"], "allow");
    }

    #[test]
    fn test_cancel_params_result() {
        let params = CancelParams {};
        let json = serde_json::to_string(&params).unwrap();
        assert_eq!(json, "{}");
        let de: CancelParams = serde_json::from_str(&json).unwrap();
        assert_eq!(de, params);

        let result = CancelResult {};
        let json = serde_json::to_string(&result).unwrap();
        assert_eq!(json, "{}");
        let de: CancelResult = serde_json::from_str(&json).unwrap();
        assert_eq!(de, result);
    }

    #[test]
    fn test_set_plan_mode_params_result() {
        let params = SetPlanModeParams { enabled: true };
        let json = serde_json::to_value(&params).unwrap();
        assert_eq!(json["enabled"], true);

        let result = SetPlanModeResult {
            status: "ok".to_string(),
            plan_mode: Some(true),
        };
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["status"], "ok");
        assert_eq!(json["plan_mode"], true);
    }

    #[test]
    fn test_steer_params_result() {
        let params = SteerParams {
            user_input: UserInput::Text("use rust".to_string()),
        };
        let json = serde_json::to_value(&params).unwrap();
        assert_eq!(json["user_input"], "use rust");

        let result = SteerResult {
            status: "steered".to_string(),
        };
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["status"], "steered");
    }

    #[test]
    fn test_replay_params_result() {
        let params = ReplayParams {};
        let json = serde_json::to_string(&params).unwrap();
        assert_eq!(json, "{}");

        let result = ReplayResult {
            status: "finished".to_string(),
            events: None,
            requests: None,
        };
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["status"], "finished");
        assert!(!json.as_object().unwrap().contains_key("events"));
    }

    #[test]
    fn test_approval_response_type_serialization() {
        assert_eq!(
            serde_json::to_value(ApprovalResponseType::Approve).unwrap(),
            "approve"
        );
        assert_eq!(
            serde_json::to_value(ApprovalResponseType::ApproveForSession).unwrap(),
            "approve_for_session"
        );
        assert_eq!(
            serde_json::to_value(ApprovalResponseType::Reject).unwrap(),
            "reject"
        );
    }

    #[test]
    fn test_event_approval_response_roundtrip() {
        let event = Event::ApprovalResponse {
            request_id: "req_1".to_string(),
            response: ApprovalResponseType::ApproveForSession,
            feedback: Some("looks good".to_string()),
        };
        let params = event.to_params().unwrap();
        assert_eq!(params.event_type, "approval_response");
        assert_eq!(params.payload["response"], "approve_for_session");
        let back = params.to_event().unwrap();
        assert_eq!(back, event);
    }

    #[test]
    fn test_event_tool_result_roundtrip() {
        let event = Event::ToolResult {
            tool_call_id: "call_1".to_string(),
            return_value: ToolReturnValue {
                is_error: false,
                output: "file contents".to_string(),
                message: "success".to_string(),
                display: Some(vec![DisplayBlock::Brief(BriefDisplayBlock {
                    text: "result".to_string(),
                })]),
                extras: None,
            },
        };
        let params = event.to_params().unwrap();
        assert_eq!(params.event_type, "tool_result");
        let back = params.to_event().unwrap();
        assert_eq!(back, event);
    }

    #[test]
    fn test_event_plan_display_roundtrip() {
        let event = Event::PlanDisplay {
            content: "plan content".to_string(),
            file_path: "/tmp/plan.md".to_string(),
        };
        let params = event.to_params().unwrap();
        assert_eq!(params.event_type, "plan_display");
        let back = params.to_event().unwrap();
        assert_eq!(back, event);
    }

    #[test]
    fn test_event_steer_input_roundtrip() {
        let event = Event::SteerInput {
            user_input: "steer me".to_string(),
        };
        let params = event.to_params().unwrap();
        assert_eq!(params.event_type, "steer_input");
        let back = params.to_event().unwrap();
        assert_eq!(back, event);
    }

    #[test]
    fn test_event_hook_triggered_roundtrip() {
        let event = Event::HookTriggered {
            event: "file_write".to_string(),
            target: "/tmp/test".to_string(),
            hook_count: 3,
        };
        let params = event.to_params().unwrap();
        assert_eq!(params.event_type, "hook_triggered");
        let back = params.to_event().unwrap();
        assert_eq!(back, event);
    }

    #[test]
    fn test_event_step_begin_roundtrip() {
        let event = Event::StepBegin { n: 42 };
        let params = event.to_params().unwrap();
        assert_eq!(params.event_type, "step_begin");
        assert_eq!(params.payload["n"], 42);
        let back = params.to_event().unwrap();
        assert_eq!(back, event);
    }

    #[test]
    fn test_event_step_interrupted_roundtrip() {
        let event = Event::StepInterrupted;
        let params = event.to_params().unwrap();
        assert_eq!(params.event_type, "step_interrupted");
        let back = params.to_event().unwrap();
        assert_eq!(back, event);
    }

    #[test]
    fn test_event_compaction_roundtrip() {
        let begin = Event::CompactionBegin;
        let params = begin.to_params().unwrap();
        assert_eq!(params.event_type, "compaction_begin");
        let back = params.to_event().unwrap();
        assert_eq!(back, begin);

        let end = Event::CompactionEnd;
        let params = end.to_params().unwrap();
        assert_eq!(params.event_type, "compaction_end");
        let back = params.to_event().unwrap();
        assert_eq!(back, end);
    }

    #[test]
    fn test_event_tool_call_part_roundtrip() {
        let event = Event::ToolCallPart {
            arguments_part: Some("{\"path\": \"/".to_string()),
        };
        let params = event.to_params().unwrap();
        assert_eq!(params.event_type, "tool_call_part");
        let back = params.to_event().unwrap();
        assert_eq!(back, event);
    }

    #[test]
    fn test_event_turn_end_roundtrip() {
        let event = Event::TurnEnd;
        let params = event.to_params().unwrap();
        assert_eq!(params.event_type, "turn_end");
        let back = params.to_event().unwrap();
        assert_eq!(back, event);
    }

    #[test]
    fn test_redact_wire_secrets_recursive() {
        let raw = json!({
            "api_key": "abc123",
            "nested": {
                "token": "tok123",
                "headers": {
                    "authorization": "Bearer abc"
                },
                "token_usage": 42
            },
            "items": [
                {"password": "pass1"},
                {"safe": "value"}
            ]
        });
        let redacted = redact_wire_secrets(&raw);

        assert_eq!(redacted["api_key"], REDACTED_SECRET);
        assert_eq!(redacted["nested"]["token"], REDACTED_SECRET);
        assert_eq!(
            redacted["nested"]["headers"]["authorization"],
            REDACTED_SECRET
        );
        assert_eq!(redacted["nested"]["token_usage"], 42);
        assert_eq!(redacted["items"][0]["password"], REDACTED_SECRET);
        assert_eq!(redacted["items"][1]["safe"], "value");
    }

    #[test]
    fn test_redact_wire_secrets_case_insensitive_and_suffix() {
        let raw = json!({
            "Authorization": "Bearer 1",
            "access_token": "tok_1",
            "client_secret": "secret_1",
            "ApiKey": "key_1",
            "safe_value": "ok"
        });
        let redacted = redact_wire_secrets(&raw);

        assert_eq!(redacted["Authorization"], REDACTED_SECRET);
        assert_eq!(redacted["access_token"], REDACTED_SECRET);
        assert_eq!(redacted["client_secret"], REDACTED_SECRET);
        assert_eq!(redacted["ApiKey"], REDACTED_SECRET);
        assert_eq!(redacted["safe_value"], "ok");
    }
}
