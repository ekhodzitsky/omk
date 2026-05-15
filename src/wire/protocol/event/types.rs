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
