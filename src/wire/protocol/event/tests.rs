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
fn test_event_content_part_roundtrip() {
    let event = Event::ContentPart {
        text: Some("Hello world".to_string()),
        chunk: None,
    };
    let params = event.to_params().unwrap();
    assert_eq!(params.event_type, "content_part");
    assert_eq!(params.payload["text"], "Hello world");
    let back = params.to_event().unwrap();
    assert_eq!(back, event);

    let event_chunk = Event::ContentPart {
        text: None,
        chunk: Some("chunk-data".to_string()),
    };
    let params_chunk = event_chunk.to_params().unwrap();
    assert_eq!(params_chunk.event_type, "content_part");
    assert_eq!(params_chunk.payload["chunk"], "chunk-data");
    let back_chunk = params_chunk.to_event().unwrap();
    assert_eq!(back_chunk, event_chunk);
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
