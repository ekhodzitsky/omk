use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::wire::protocol::content::DisplayBlock;

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
    /// Return a stable snake_case event type for matching across Kimi releases.
    pub fn normalized_event_type(&self) -> String {
        normalize_wire_kind(&self.event_type)
    }

    /// Convert raw wire params into a typed event.
    pub fn to_event(&self) -> Result<Event, serde_json::Error> {
        let mut value = self.payload.clone();
        if let Value::Object(ref mut map) = value {
            map.insert(
                "type".to_string(),
                Value::String(self.normalized_event_type()),
            );
        }
        serde_json::from_value(value)
    }
}

fn normalize_wire_kind(raw: &str) -> String {
    let mut normalized = String::with_capacity(raw.len() + 4);
    let mut previous_was_separator = true;

    for ch in raw.chars() {
        if ch == '-' || ch == '_' || ch == ' ' {
            if !normalized.ends_with('_') {
                normalized.push('_');
            }
            previous_was_separator = true;
            continue;
        }

        if ch.is_ascii_uppercase() {
            if !previous_was_separator && !normalized.ends_with('_') {
                normalized.push('_');
            }
            normalized.push(ch.to_ascii_lowercase());
            previous_was_separator = false;
        } else {
            normalized.push(ch.to_ascii_lowercase());
            previous_was_separator = false;
        }
    }

    normalized.trim_matches('_').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::protocol::content::{BriefDisplayBlock, DisplayBlock};
    use serde_json::json;

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
}
