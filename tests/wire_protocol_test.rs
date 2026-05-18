use omk::wire::client::{
    process_messages, ProcessWireClient, WireClient, WireMessage as ClientWireMessage,
};
use omk::wire::protocol::*;
use serde_json::json;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::sync::{Arc, Mutex};
use tempfile::TempDir;

#[test]
fn test_jsonrpc_request_serialization() {
    let req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        method: "initialize".to_string(),
        id: "1".to_string(),
        params: json!({"protocol_version": "1.9"}),
    };
    let json = serde_json::to_string(&req).unwrap();
    assert!(json.contains("\"jsonrpc\":\"2.0\""));
    assert!(json.contains("\"method\":\"initialize\""));
    assert!(json.contains("\"id\":\"1\""));
}

#[test]
fn test_jsonrpc_error_response() {
    let resp = JsonRpcErrorResponse {
        jsonrpc: "2.0".to_string(),
        id: "1".to_string(),
        error: JsonRpcError {
            code: -32601,
            message: "Method not found".to_string(),
            data: None,
        },
    };
    let json = serde_json::to_string(&resp).unwrap();
    assert!(json.contains("\"code\":-32601"));
    assert!(json.contains("\"message\":\"Method not found\""));
}

#[test]
fn test_initialize_params_roundtrip() {
    let params = InitializeParams {
        protocol_version: "1.9".to_string(),
        client: Some(ClientInfo {
            name: "omk".to_string(),
            version: Some("0.2.5".to_string()),
        }),
        external_tools: None,
        capabilities: Some(ClientCapabilities {
            supports_question: Some(true),
            supports_plan_mode: Some(true),
        }),
        hooks: Some(vec![WireHookSubscription {
            id: "pre-tool".to_string(),
            event: "PreToolUse".to_string(),
            matcher: Some("Shell".to_string()),
            timeout: Some(30),
        }]),
    };
    let json = serde_json::to_string(&params).unwrap();
    let parsed: InitializeParams = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.protocol_version, "1.9");
    assert!(parsed.client.is_some());
    assert!(json.contains("\"protocol_version\""));
    assert!(json.contains("\"supports_question\""));
    assert!(json.contains("\"supports_plan_mode\""));
}

#[test]
fn test_initialize_result_roundtrip_wire_19() {
    let result = InitializeResult {
        protocol_version: "1.9".to_string(),
        server: Some(json!({"name":"Kimi Code CLI","version":"1.41.0"})),
        slash_commands: Some(vec![]),
        external_tools: None,
        capabilities: Some(json!({"supports_question": true})),
        hooks: None,
    };
    let json = serde_json::to_string(&result).unwrap();
    let parsed: InitializeResult = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.protocol_version, "1.9");
}

#[test]
fn test_initialize_result_accepts_kimi_141_hooks_object() {
    let json = json!({
        "protocol_version": "1.9",
        "server": {"name": "Kimi Code CLI", "version": "1.41.0"},
        "slash_commands": [],
        "capabilities": {"supports_question": true},
        "hooks": {
            "supported_events": ["PreToolUse", "PostToolUse", "Stop"],
            "configured": {}
        }
    });

    let parsed: InitializeResult = serde_json::from_value(json).unwrap();
    assert_eq!(parsed.protocol_version, "1.9");
    assert_eq!(
        parsed
            .hooks
            .as_ref()
            .and_then(|hooks| hooks.get("supported_events"))
            .and_then(|events| events.as_array())
            .map(Vec::len),
        Some(3)
    );
}

#[test]
fn test_prompt_result_finished() {
    let result = PromptResult {
        status: "finished".to_string(),
        steps: None,
    };
    let json = serde_json::to_string(&result).unwrap();
    assert!(json.contains("\"status\":\"finished\""));
}

#[test]
fn test_prompt_result_accepts_step_count_and_legacy_trace() {
    let counted: PromptResult =
        serde_json::from_value(json!({"status":"max_steps","steps":3})).unwrap();
    assert_eq!(counted.steps, Some(PromptSteps::Count(3)));

    let legacy: PromptResult =
        serde_json::from_value(json!({"status":"ok","steps":[{"n":1}]})).unwrap();
    assert_eq!(
        legacy.steps,
        Some(PromptSteps::LegacyTrace(vec![json!({"n":1})]))
    );
}

#[test]
fn test_event_params_turn_begin() {
    let event = EventParams {
        event_type: "TurnBegin".to_string(),
        payload: json!({"user_input": "Hello"}),
    };
    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("\"type\":\"TurnBegin\""));
}

#[test]
fn test_event_params_normalizes_pascal_case_events() {
    let event = EventParams {
        event_type: "ContentPart".to_string(),
        payload: json!({"type": "text", "text": "Hello"}),
    };
    assert_eq!(event.normalized_event_type(), "content_part");

    let turn_end = EventParams {
        event_type: "TurnEnd".to_string(),
        payload: json!({}),
    };
    assert_eq!(turn_end.normalized_event_type(), "turn_end");
    assert!(matches!(turn_end.to_event().unwrap(), Event::TurnEnd));
}

#[test]
fn test_content_part_text() {
    let part = ContentPart::Text(TextPart {
        text: "Hello".to_string(),
    });
    let json = serde_json::to_string(&part).unwrap();
    assert!(json.contains("\"type\":\"text\""));
    assert!(json.contains("\"text\":\"Hello\""));
}

#[test]
fn test_content_part_think() {
    let part = ContentPart::Think(ThinkPart {
        think: "Reasoning...".to_string(),
        encrypted: None,
    });
    let json = serde_json::to_string(&part).unwrap();
    assert!(json.contains("\"type\":\"think\""));
}

#[test]
fn test_approval_request_response() {
    let req = ApprovalRequest {
        id: "app-1".to_string(),
        tool_call_id: "tc-1".to_string(),
        sender: "Shell".to_string(),
        action: "run command".to_string(),
        description: "ls".to_string(),
        display: None,
        source_kind: None,
        source_id: None,
        agent_id: None,
        subagent_type: None,
        source_description: None,
    };
    let json = serde_json::to_string(&req).unwrap();
    let parsed: ApprovalRequest = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.sender, "Shell");
}

#[test]
fn test_tool_call_serialization() {
    let tc = Event::ToolCall {
        id: "tc-1".to_string(),
        function: ToolCallFunction {
            name: "Shell".to_string(),
            arguments: Some("{\"command\":\"ls\"}".to_string()),
        },
        extras: None,
    };
    let json = serde_json::to_string(&tc).unwrap();
    assert!(json.contains("\"name\":\"Shell\""));
}

#[test]
fn test_display_block_diff() {
    let block = DisplayBlock::Diff(DiffDisplayBlock {
        path: "src/lib.rs".to_string(),
        old_text: "fn old() {}".to_string(),
        new_text: "fn new() {}".to_string(),
    });
    let json = serde_json::to_string(&block).unwrap();
    assert!(json.contains("\"type\":\"diff\""));
}

#[test]
fn test_display_block_todo() {
    let block = DisplayBlock::Todo(TodoDisplayBlock {
        items: vec![
            TodoDisplayItem {
                title: "Fix tests".to_string(),
                status: TodoStatus::InProgress,
            },
            TodoDisplayItem {
                title: "Update docs".to_string(),
                status: TodoStatus::Pending,
            },
        ],
    });
    let json = serde_json::to_string(&block).unwrap();
    assert!(json.contains("\"type\":\"todo\""));
    assert!(json.contains("\"in_progress\""));
}

#[test]
fn test_status_update_with_tokens() {
    let su = Event::StatusUpdate {
        context_usage: Some(0.75),
        context_tokens: Some(15000),
        max_context_tokens: Some(20000),
        token_usage: Some(3500),
        message_id: Some("msg-1".to_string()),
        plan_mode: Some(false),
    };
    let json = serde_json::to_string(&su).unwrap();
    assert!(json.contains("\"context_usage\":0.75"));
}

#[test]
fn test_wire_message_parsing_event() {
    let json = r#"{"jsonrpc":"2.0","method":"event","params":{"type":"TurnBegin","payload":{"user_input":"Hello"}}}"#;
    let msg: WireMessage = serde_json::from_str(json).unwrap();
    assert_eq!(msg.jsonrpc, "2.0");
    assert_eq!(msg.method.as_deref(), Some("event"));
}

#[test]
fn test_wire_message_parsing_request() {
    let json = r#"{"jsonrpc":"2.0","method":"request","id":"req-1","params":{"type":"ApprovalRequest","payload":{"id":"app-1","tool_call_id":"tc-1","sender":"Shell","action":"run","description":"ls"}}}"#;
    let msg: WireMessage = serde_json::from_str(json).unwrap();
    assert_eq!(msg.jsonrpc, "2.0");
    assert_eq!(msg.id.as_deref(), Some("req-1"));
    assert_eq!(msg.method.as_deref(), Some("request"));
}

#[test]
fn test_wire_message_parsing_response() {
    let json = r#"{"jsonrpc":"2.0","id":"1","result":{"status":"finished"}}"#;
    let msg: WireMessage = serde_json::from_str(json).unwrap();
    assert_eq!(msg.jsonrpc, "2.0");
    assert_eq!(msg.id.as_deref(), Some("1"));
    assert!(msg.result.is_some());
}

#[test]
fn test_redact_wire_secrets_for_nested_payloads() {
    let raw = json!({
        "authorization": "Bearer secret",
        "nested": {
            "api_key": "abc123",
            "safe": "ok"
        },
        "items": [
            {"password": "pass"},
            {"name": "visible"}
        ]
    });

    let redacted = redact_wire_secrets(&raw);
    assert_eq!(redacted["authorization"], "[REDACTED]");
    assert_eq!(redacted["nested"]["api_key"], "[REDACTED]");
    assert_eq!(redacted["nested"]["safe"], "ok");
    assert_eq!(redacted["items"][0]["password"], "[REDACTED]");
    assert_eq!(redacted["items"][1]["name"], "visible");
}

#[tokio::test]
async fn test_mock_wire_session() {
    let tmp = TempDir::new().unwrap();
    let script_path = tmp.path().join("mock_wire.sh");

    let script = r#"#!/bin/bash
echo '{"jsonrpc":"2.0","id":"req-1","result":{"protocol_version":"1.9"}}'
echo '{"jsonrpc":"2.0","method":"event","params":{"type":"TurnBegin","payload":{"user_input":"Hello"}}}'
echo '{"jsonrpc":"2.0","id":"req-2","result":{"status":"finished"}}'
cat > /dev/null
"#;

    fs::write(&script_path, script).unwrap();
    let mut perms = fs::metadata(&script_path).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&script_path, perms).unwrap();

    let mut client =
        ProcessWireClient::spawn(script_path.to_str().unwrap(), None, None, None).await.unwrap();

    // Send initialize request and read response
    let init_result = client
        .initialize(InitializeParams {
            protocol_version: "1.9".to_string(),
            client: None,
            external_tools: None,
            capabilities: None,
            hooks: None,
        })
        .await
        .unwrap();
    assert_eq!(init_result.protocol_version, "1.9");

    // Read event
    let msg = client.read_message().await.unwrap();
    match msg {
        omk::wire::client::WireMessage::Event(ev) => {
            assert_eq!(ev.params.event_type, "TurnBegin");
        }
        other => panic!("Expected event, got {:?}", other),
    }

    // Send prompt and read result
    let prompt_result = client.prompt("Hello").await.unwrap();
    assert_eq!(prompt_result.status, "finished");

    client.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_replay_roundtrip() {
    let tmp = TempDir::new().unwrap();
    let script_path = tmp.path().join("mock_replay.sh");

    let script = r#"#!/bin/bash
read -r line
echo '{"jsonrpc":"2.0","id":"req-1","result":{"protocol_version":"1.9"}}'
read -r line
echo '{"jsonrpc":"2.0","id":"req-2","result":{"status":"finished","events":[{"type":"text","payload":{"text":"hello"}}],"requests":[{"type":"ToolCallRequest","payload":{"name":"Shell"}}]}}'
cat > /dev/null
"#;

    fs::write(&script_path, script).unwrap();
    let mut perms = fs::metadata(&script_path).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&script_path, perms).unwrap();

    let mut client =
        ProcessWireClient::spawn(script_path.to_str().unwrap(), None, None, None).await.unwrap();

    let _ = client
        .initialize(InitializeParams {
            protocol_version: "1.9".to_string(),
            client: None,
            external_tools: None,
            capabilities: None,
            hooks: None,
        })
        .await
        .unwrap();

    let replay = client.replay().await.unwrap();
    assert_eq!(replay.status, "finished");
    assert_eq!(replay.events.as_ref().unwrap().len(), 1);
    assert_eq!(replay.requests.as_ref().unwrap().len(), 1);

    client.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_process_messages_skips_unknown_method_and_unknown_event_kind() {
    let tmp = TempDir::new().unwrap();
    let script_path = tmp.path().join("mock_unknown_method_event.sh");

    let script = r#"#!/bin/bash
echo '{"jsonrpc":"2.0","method":"tool_call","id":"req-unknown-method","params":{"type":"ApprovalRequest","payload":{"id":"app-1","tool_call_id":"tc-1","sender":"Shell","action":"run","description":"ls"}}}'
echo '{"jsonrpc":"2.0","method":"event","params":{"type":"UnknownEventKind","payload":{"foo":"bar"}}}'
echo '{"jsonrpc":"2.0","method":"event","params":{"type":"turn_begin","payload":{"user_input":"Hello"}}}'
"#;

    fs::write(&script_path, script).unwrap();
    let mut perms = fs::metadata(&script_path).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&script_path, perms).unwrap();

    let mut client =
        ProcessWireClient::spawn(script_path.to_str().unwrap(), None, None, None).await.unwrap();
    let seen_events = Arc::new(Mutex::new(Vec::<String>::new()));
    let seen_events_for_handler = seen_events.clone();

    process_messages(&mut client, move |msg| {
        let seen_events = seen_events_for_handler.clone();
        async move {
            if let ClientWireMessage::Event(ev) = msg {
                seen_events.lock().unwrap().push(ev.params.event_type);
            }
            Ok(None)
        }
    })
    .await
    .unwrap();

    let seen = seen_events.lock().unwrap().clone();
    assert_eq!(seen, vec!["turn_begin".to_string()]);

    client.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_process_messages_unknown_request_type_sends_error_and_continues() {
    let tmp = TempDir::new().unwrap();
    let script_path = tmp.path().join("mock_unknown_request_type.sh");
    let capture_path = tmp.path().join("stdin_capture.jsonl");
    let script = format!(
        r#"#!/bin/bash
capture="{capture}"
echo '{{"jsonrpc":"2.0","method":"request","id":"req-unknown-type","params":{{"type":"AlienRequest","payload":{{"x":1}}}}}}'
read -r line
echo "$line" > "$capture"
echo '{{"jsonrpc":"2.0","method":"event","params":{{"type":"turn_begin","payload":{{"user_input":"Hello"}}}}}}'
"#,
        capture = capture_path.display()
    );

    fs::write(&script_path, script).unwrap();
    let mut perms = fs::metadata(&script_path).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&script_path, perms).unwrap();

    let mut client =
        ProcessWireClient::spawn(script_path.to_str().unwrap(), None, None, None).await.unwrap();
    let seen_events = Arc::new(Mutex::new(Vec::<String>::new()));
    let seen_events_for_handler = seen_events.clone();

    process_messages(&mut client, move |msg| {
        let seen_events = seen_events_for_handler.clone();
        async move {
            if let ClientWireMessage::Event(ev) = msg {
                seen_events.lock().unwrap().push(ev.params.event_type);
            }
            Ok(None)
        }
    })
    .await
    .unwrap();

    let seen = seen_events.lock().unwrap().clone();
    assert_eq!(seen, vec!["turn_begin".to_string()]);

    let raw = fs::read_to_string(&capture_path).unwrap();
    let response: JsonRpcErrorResponse = serde_json::from_str(raw.trim()).unwrap();
    assert_eq!(response.id, "req-unknown-type");
    assert_eq!(response.error.code, -32601);
    assert_eq!(response.error.message, "Unknown request type");

    client.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_steer_and_set_plan_mode_roundtrip() {
    let tmp = TempDir::new().unwrap();
    let script_path = tmp.path().join("mock_control.sh");

    let script = r#"#!/bin/bash
read -r line
echo '{"jsonrpc":"2.0","id":"req-1","result":{"protocol_version":"1.9"}}'
read -r line
echo '{"jsonrpc":"2.0","id":"req-2","result":{"status":"steered"}}'
read -r line
echo '{"jsonrpc":"2.0","id":"req-3","result":{"status":"ok","plan_mode":true}}'
cat > /dev/null
"#;

    fs::write(&script_path, script).unwrap();
    let mut perms = fs::metadata(&script_path).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&script_path, perms).unwrap();

    let mut client =
        ProcessWireClient::spawn(script_path.to_str().unwrap(), None, None, None).await.unwrap();

    let _ = client
        .initialize(InitializeParams {
            protocol_version: "1.9".to_string(),
            client: None,
            external_tools: None,
            capabilities: None,
            hooks: None,
        })
        .await
        .unwrap();

    let steer = client.steer("prefer smaller diffs").await.unwrap();
    assert_eq!(steer.status, "steered");

    let plan = client.set_plan_mode(true).await.unwrap();
    assert_eq!(plan.status, "ok");
    assert_eq!(plan.plan_mode, Some(true));

    client.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_cancel_roundtrip() {
    let tmp = TempDir::new().unwrap();
    let script_path = tmp.path().join("mock_cancel.sh");

    let script = r#"#!/bin/bash
read -r line
echo '{"jsonrpc":"2.0","id":"req-1","result":{"protocol_version":"1.9"}}'
read -r line
echo '{"jsonrpc":"2.0","id":"req-2","result":{}}'
cat > /dev/null
"#;

    fs::write(&script_path, script).unwrap();
    let mut perms = fs::metadata(&script_path).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&script_path, perms).unwrap();

    let mut client =
        ProcessWireClient::spawn(script_path.to_str().unwrap(), None, None, None).await.unwrap();

    let _ = client
        .initialize(InitializeParams {
            protocol_version: "1.9".to_string(),
            client: None,
            external_tools: None,
            capabilities: None,
            hooks: None,
        })
        .await
        .unwrap();

    client.cancel().await.unwrap();
    client.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_wire_error_response_is_actionable() {
    let tmp = TempDir::new().unwrap();
    let script_path = tmp.path().join("mock_error.sh");

    let script = r#"#!/bin/bash
read -r line
echo '{"jsonrpc":"2.0","id":"req-1","result":{"protocol_version":"1.9"}}'
read -r line
echo '{"jsonrpc":"2.0","id":"req-2","error":{"code":-32601,"message":"Method not found"}}'
cat > /dev/null
"#;

    fs::write(&script_path, script).unwrap();
    let mut perms = fs::metadata(&script_path).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&script_path, perms).unwrap();

    let mut client =
        ProcessWireClient::spawn(script_path.to_str().unwrap(), None, None, None).await.unwrap();

    let _ = client
        .initialize(InitializeParams {
            protocol_version: "1.9".to_string(),
            client: None,
            external_tools: None,
            capabilities: None,
            hooks: None,
        })
        .await
        .unwrap();

    let err = client.prompt("hello").await.unwrap_err().to_string();
    assert!(err.contains("Wire request failed"));
    assert!(err.contains("Method not found"));
    assert!(err.contains("-32601"));

    client.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_initialize_fallback_no_handshake() {
    let tmp = TempDir::new().unwrap();
    let script_path = tmp.path().join("mock_no_handshake.sh");

    let script = r#"#!/bin/bash
read -r line
echo '{"jsonrpc":"2.0","id":"req-1","error":{"code":-32601,"message":"Method not found"}}'
read -r line
echo '{"jsonrpc":"2.0","id":"req-2","result":{"status":"ok","steps":[{"n":1}]}}'
cat > /dev/null
"#;

    fs::write(&script_path, script).unwrap();
    let mut perms = fs::metadata(&script_path).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&script_path, perms).unwrap();

    let mut client =
        ProcessWireClient::spawn(script_path.to_str().unwrap(), None, None, None).await.unwrap();

    let init_result = client
        .initialize(InitializeParams {
            protocol_version: "1.9".to_string(),
            client: None,
            external_tools: None,
            capabilities: None,
            hooks: None,
        })
        .await
        .unwrap();
    assert!(client.is_handshake_done());
    assert_eq!(init_result.protocol_version, "legacy/no-handshake");

    let prompt_result = client.prompt("hello").await.unwrap();
    assert_eq!(prompt_result.status, "ok");

    client.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_wire_client_startup_failure() {
    let tmp = TempDir::new().unwrap();
    let script_path = tmp.path().join("fail_startup.sh");
    let script = r#"#!/bin/bash
exit 1
"#;
    fs::write(&script_path, script).unwrap();
    let mut perms = fs::metadata(&script_path).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&script_path, perms).unwrap();

    let mut client =
        ProcessWireClient::spawn(script_path.to_str().unwrap(), None, None, None).await.unwrap();

    let result = client
        .initialize(InitializeParams {
            protocol_version: "1.9".to_string(),
            client: None,
            external_tools: None,
            capabilities: None,
            hooks: None,
        })
        .await;

    assert!(result.is_err());
    client.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_wire_client_malformed_output() {
    let tmp = TempDir::new().unwrap();
    let script_path = tmp.path().join("malformed.sh");
    let script = r#"#!/bin/bash
read -r line
echo 'this is definitely not json'
cat > /dev/null
"#;
    fs::write(&script_path, script).unwrap();
    let mut perms = fs::metadata(&script_path).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&script_path, perms).unwrap();

    let mut client =
        ProcessWireClient::spawn(script_path.to_str().unwrap(), None, None, None).await.unwrap();

    let result = client
        .initialize(InitializeParams {
            protocol_version: "1.9".to_string(),
            client: None,
            external_tools: None,
            capabilities: None,
            hooks: None,
        })
        .await;

    assert!(result.is_err());
    client.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_wire_client_eof_after_partial() {
    let tmp = TempDir::new().unwrap();
    let script_path = tmp.path().join("partial.sh");
    let script = r#"#!/bin/bash
read -r line
echo '{"jsonrpc":"2.0","id":"req-1","result":'
"#;
    fs::write(&script_path, script).unwrap();
    let mut perms = fs::metadata(&script_path).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&script_path, perms).unwrap();

    let mut client =
        ProcessWireClient::spawn(script_path.to_str().unwrap(), None, None, None).await.unwrap();

    let result = client
        .initialize(InitializeParams {
            protocol_version: "1.9".to_string(),
            client: None,
            external_tools: None,
            capabilities: None,
            hooks: None,
        })
        .await;

    assert!(result.is_err());
    client.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_wire_client_read_message_timeout_when_turn_stalls() {
    use std::time::Duration;

    let tmp = TempDir::new().unwrap();
    let script_path = tmp.path().join("stall_after_prompt.sh");
    let script = r#"#!/bin/bash
read -r line
echo '{"jsonrpc":"2.0","id":"req-1","result":{"protocol_version":"1.9"}}'
read -r line
echo '{"jsonrpc":"2.0","id":"req-2","result":{"status":"ok","steps":[{"n":1}]}}'
sleep 10
"#;
    fs::write(&script_path, script).unwrap();
    let mut perms = fs::metadata(&script_path).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&script_path, perms).unwrap();

    let mut client =
        ProcessWireClient::spawn(script_path.to_str().unwrap(), None, None, None).await.unwrap();
    client
        .initialize(InitializeParams {
            protocol_version: "1.9".to_string(),
            client: None,
            external_tools: None,
            capabilities: None,
            hooks: None,
        })
        .await
        .unwrap();
    let _ = client.prompt("stall please").await.unwrap();

    let err = client
        .read_message_timeout(Duration::from_millis(150))
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("timed out"));
    assert!(err.contains("150ms"));

    client.shutdown().await.unwrap();
}
