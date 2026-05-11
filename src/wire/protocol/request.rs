use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::wire::protocol::event::HookAction;
use crate::wire::protocol::{
    ApprovalResponseType, BriefDisplayBlock, DisplayBlock, ToolReturnValue,
};

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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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
}
