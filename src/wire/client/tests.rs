use super::*;
use crate::wire::protocol::PromptSteps;

#[test]
fn test_wire_message_parsing_event() {
    let json = r#"{"jsonrpc":"2.0","method":"event","params":{"type":"thinking","payload":{"chunk":"hello"}}}"#;
    let msg: WireMessage = serde_json::from_str(json).unwrap();
    match msg {
        WireMessage::Event(notif) => {
            assert_eq!(notif.method, "event");
            assert_eq!(notif.params.event_type, "thinking");
            assert_eq!(notif.params.payload["chunk"], "hello");
        }
        other => panic!("Expected event, got {:?}", other),
    }
}

#[test]
fn test_wire_message_parsing_request() {
    let json = r#"{"jsonrpc":"2.0","method":"tool_call","id":"req-1","params":{"type":"read_file","payload":{"path":"/tmp/test"}}}"#;
    let msg: WireMessage = serde_json::from_str(json).unwrap();
    match msg {
        WireMessage::Request(req) => {
            assert_eq!(req.method, "tool_call");
            assert_eq!(req.id, "req-1");
            assert_eq!(req.params.request_type, "read_file");
        }
        other => panic!("Expected request, got {:?}", other),
    }
}

#[test]
fn test_wire_message_parsing_success_response() {
    let json = r#"{"jsonrpc":"2.0","id":"req-1","result":{"status":"ok"}}"#;
    let msg: WireMessage = serde_json::from_str(json).unwrap();
    match msg {
        WireMessage::SuccessResponse(resp) => {
            assert_eq!(resp.id, "req-1");
            assert_eq!(resp.result["status"], "ok");
        }
        other => panic!("Expected success response, got {:?}", other),
    }
}

#[test]
fn test_wire_message_parsing_error_response() {
    let json =
        r#"{"jsonrpc":"2.0","id":"req-1","error":{"code":-32600,"message":"Invalid Request"}}"#;
    let msg: WireMessage = serde_json::from_str(json).unwrap();
    match msg {
        WireMessage::ErrorResponse(resp) => {
            assert_eq!(resp.id, "req-1");
            assert_eq!(resp.error.code, -32600);
            assert_eq!(resp.error.message, "Invalid Request");
        }
        other => panic!("Expected error response, got {:?}", other),
    }
}

#[tokio::test]
async fn test_wire_client_spawn() {
    let tmp = tempfile::tempdir().unwrap();
    let script = tmp.path().join("mock-true");
    let script_content = r#"#!/bin/bash
exit 0
"#;
    tokio::fs::write(&script, script_content).await.unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = tokio::fs::metadata(&script).await.unwrap().permissions();
        perms.set_mode(0o755);
        tokio::fs::set_permissions(&script, perms).await.unwrap();
    }

    let client = WireClient::spawn(script.to_str().unwrap(), None, None, None);
    assert!(client.is_ok());
    let client = client.unwrap();
    client.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_roundtrip_send_request_read_response() {
    let tmp = tempfile::tempdir().unwrap();
    let script = tmp.path().join("mock-wire");
    let script_content = r#"#!/bin/bash
read -r line
echo '{"jsonrpc":"2.0","id":"req-1","result":{"status":"ok","steps":[{"n":1}]}}'
"#;
    tokio::fs::write(&script, script_content).await.unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = tokio::fs::metadata(&script).await.unwrap().permissions();
        perms.set_mode(0o755);
        tokio::fs::set_permissions(&script, perms).await.unwrap();
    }

    let mut client = WireClient::spawn(script.to_str().unwrap(), None, None, None).unwrap();

    let result = client.prompt("hello").await.unwrap();
    assert_eq!(result.status, "ok");
    match result.steps.unwrap() {
        PromptSteps::LegacyTrace(steps) => {
            assert_eq!(steps.len(), 1);
            assert_eq!(steps[0]["n"], 1);
        }
        other => panic!("expected legacy prompt trace, got {:?}", other),
    }

    client.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_prompt_buffers_events_that_arrive_before_response() {
    let tmp = tempfile::tempdir().unwrap();
    let script = tmp.path().join("mock-wire-event-first");
    let script_content = r#"#!/bin/bash
read -r line
echo '{"jsonrpc":"2.0","method":"event","params":{"type":"turn_begin","payload":{"user_input":"hello"}}}'
echo '{"jsonrpc":"2.0","id":"req-1","result":{"status":"ok"}}'
"#;
    tokio::fs::write(&script, script_content).await.unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = tokio::fs::metadata(&script).await.unwrap().permissions();
        perms.set_mode(0o755);
        tokio::fs::set_permissions(&script, perms).await.unwrap();
    }

    let mut client = WireClient::spawn(script.to_str().unwrap(), None, None, None).unwrap();

    let result = client.prompt("hello").await.unwrap();
    assert_eq!(result.status, "ok");

    let buffered = client.read_message().await.unwrap();
    match buffered {
        WireMessage::Event(ev) => assert_eq!(ev.params.event_type, "turn_begin"),
        other => panic!("expected buffered event, got {:?}", other),
    }

    client.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_prompt_waits_for_matching_response_id() {
    let tmp = tempfile::tempdir().unwrap();
    let script = tmp.path().join("mock-wire-interleaved-response");
    let script_content = r#"#!/bin/bash
read -r line
echo '{"jsonrpc":"2.0","id":"req-999","result":{"status":"wrong"}}'
echo '{"jsonrpc":"2.0","method":"event","params":{"type":"turn_begin","payload":{"user_input":"hello"}}}'
echo '{"jsonrpc":"2.0","id":"req-1","result":{"status":"ok"}}'
"#;
    tokio::fs::write(&script, script_content).await.unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = tokio::fs::metadata(&script).await.unwrap().permissions();
        perms.set_mode(0o755);
        tokio::fs::set_permissions(&script, perms).await.unwrap();
    }

    let mut client = WireClient::spawn(script.to_str().unwrap(), None, None, None).unwrap();

    let result = client.prompt("hello").await.unwrap();
    assert_eq!(result.status, "ok");

    let buffered = client.read_message().await.unwrap();
    match buffered {
        WireMessage::SuccessResponse(resp) => assert_eq!(resp.id, "req-999"),
        other => panic!("expected buffered non-matching response, got {:?}", other),
    }

    client.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_start_prompt_allows_streaming_before_response() {
    let tmp = tempfile::tempdir().unwrap();
    let script = tmp.path().join("mock-wire-stream-before-response");
    let script_content = r#"#!/bin/bash
read -r line
echo '{"jsonrpc":"2.0","method":"event","params":{"type":"turn_begin","payload":{"user_input":"hello"}}}'
sleep 1
"#;
    tokio::fs::write(&script, script_content).await.unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = tokio::fs::metadata(&script).await.unwrap().permissions();
        perms.set_mode(0o755);
        tokio::fs::set_permissions(&script, perms).await.unwrap();
    }

    let mut client = WireClient::spawn(script.to_str().unwrap(), None, None, None).unwrap();

    let id = client.start_prompt("hello").await.unwrap();
    assert_eq!(id, "req-1");

    let msg = client.read_message().await.unwrap();
    match msg {
        WireMessage::Event(ev) => assert_eq!(ev.params.event_type, "turn_begin"),
        other => panic!("expected streaming event, got {:?}", other),
    }

    client.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_send_response_and_error() {
    let tmp = tempfile::tempdir().unwrap();
    let script = tmp.path().join("mock-wire-responder");
    let script_content = r#"#!/bin/bash
read -r line
read -r line
"#;
    tokio::fs::write(&script, script_content).await.unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = tokio::fs::metadata(&script).await.unwrap().permissions();
        perms.set_mode(0o755);
        tokio::fs::set_permissions(&script, perms).await.unwrap();
    }

    let mut client = WireClient::spawn(script.to_str().unwrap(), None, None, None).unwrap();

    client
        .send_response("req-42", serde_json::json!({"ok": true}))
        .await
        .unwrap();

    client
        .send_error("req-43", -32600, "Invalid Request")
        .await
        .unwrap();

    client.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_process_messages_loop() {
    let tmp = tempfile::tempdir().unwrap();
    let script = tmp.path().join("mock-wire-events");
    let script_content = r#"#!/bin/bash
echo '{"jsonrpc":"2.0","method":"event","params":{"type":"turn_begin","payload":{"user_input":"hello"}}}'
"#;
    tokio::fs::write(&script, script_content).await.unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = tokio::fs::metadata(&script).await.unwrap().permissions();
        perms.set_mode(0o755);
        tokio::fs::set_permissions(&script, perms).await.unwrap();
    }

    let mut client = WireClient::spawn(script.to_str().unwrap(), None, None, None).unwrap();

    let seen_event = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let seen_clone = seen_event.clone();
    process_messages(&mut client, move |msg| {
        let seen = seen_clone.clone();
        async move {
            if let WireMessage::Event(ev) = msg {
                if ev.params.event_type == "turn_begin" {
                    seen.store(true, std::sync::atomic::Ordering::SeqCst);
                }
            }
            Ok(None)
        }
    })
    .await
    .unwrap();

    assert!(seen_event.load(std::sync::atomic::Ordering::SeqCst));
    client.shutdown().await.unwrap();
}
