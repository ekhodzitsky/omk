use omk::wire::client::WireClient;
use omk::wire::protocol::*;
use serde_json::json;
use std::fs;
use std::os::unix::fs::PermissionsExt;
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
fn test_prompt_result_finished() {
    let result = PromptResult {
        status: "finished".to_string(),
        steps: None,
    };
    let json = serde_json::to_string(&result).unwrap();
    assert!(json.contains("\"status\":\"finished\""));
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

#[tokio::test]
async fn test_mock_wire_session() {
    let tmp = TempDir::new().unwrap();
    let script_path = tmp.path().join("mock_wire.sh");

    let script = r#"#!/bin/bash
echo '{"jsonrpc":"2.0","id":"init-1","result":{"protocol_version":"1.9"}}'
echo '{"jsonrpc":"2.0","method":"event","params":{"type":"TurnBegin","payload":{"user_input":"Hello"}}}'
echo '{"jsonrpc":"2.0","id":"prompt-1","result":{"status":"finished"}}'
cat > /dev/null
"#;

    fs::write(&script_path, script).unwrap();
    let mut perms = fs::metadata(&script_path).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&script_path, perms).unwrap();

    let mut client = WireClient::spawn(script_path.to_str().unwrap(), None, None, None).unwrap();

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

    let mut client = WireClient::spawn(script_path.to_str().unwrap(), None, None, None).unwrap();

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

    let mut client = WireClient::spawn(script_path.to_str().unwrap(), None, None, None).unwrap();

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

    let mut client = WireClient::spawn(script_path.to_str().unwrap(), None, None, None).unwrap();

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

    let mut client = WireClient::spawn(script_path.to_str().unwrap(), None, None, None).unwrap();

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
