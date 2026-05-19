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
async fn test_mock_wire_client_roundtrip() {
    use crate::test_helpers::MockWireClient;

    let mut client = MockWireClient::new();
    client
        .inject(WireMessage::SuccessResponse(
            crate::wire::protocol::JsonRpcSuccessResponse {
                jsonrpc: "2.0".to_string(),
                id: "req-1".to_string(),
                result: serde_json::json!({"status":"ok","steps":[{"n":1}]}),
            },
        ))
        .await;

    let result = client.prompt("hello").await.unwrap();
    assert_eq!(result.status, "ok");

    let outgoing = client.drain().await;
    assert_eq!(outgoing.len(), 1);
    assert_eq!(outgoing[0]["method"], "prompt");
}

#[tokio::test]
async fn test_roundtrip_send_request_read_response() {
    let mut client = InMemoryWireClient::new();
    client
        .inject(WireMessage::SuccessResponse(
            crate::wire::protocol::JsonRpcSuccessResponse {
                jsonrpc: "2.0".to_string(),
                id: "req-1".to_string(),
                result: serde_json::json!({"status":"ok","steps":[{"n":1}]}),
            },
        ))
        .await;

    let result = client.prompt("hello").await.unwrap();
    assert_eq!(result.status, "ok");
    match result.steps.unwrap() {
        PromptSteps::LegacyTrace(steps) => {
            assert_eq!(steps.len(), 1);
            assert_eq!(steps[0]["n"], 1);
        }
        other => panic!("expected legacy prompt trace, got {:?}", other),
    }
}

#[tokio::test]
async fn test_prompt_buffers_events_that_arrive_before_response() {
    let mut client = InMemoryWireClient::new();
    client
        .inject(WireMessage::Event(
            crate::wire::protocol::JsonRpcNotification {
                jsonrpc: "2.0".to_string(),
                method: "event".to_string(),
                params: crate::wire::protocol::EventParams {
                    event_type: "turn_begin".to_string(),
                    payload: serde_json::json!({"user_input":"hello"}),
                },
            },
        ))
        .await;
    client
        .inject(WireMessage::SuccessResponse(
            crate::wire::protocol::JsonRpcSuccessResponse {
                jsonrpc: "2.0".to_string(),
                id: "req-1".to_string(),
                result: serde_json::json!({"status":"ok"}),
            },
        ))
        .await;

    let result = client.prompt("hello").await.unwrap();
    assert_eq!(result.status, "ok");

    let buffered = client.read_message().await.unwrap();
    match buffered {
        WireMessage::Event(ev) => assert_eq!(ev.params.event_type, "turn_begin"),
        other => panic!("expected buffered event, got {:?}", other),
    }
}

#[tokio::test]
async fn test_prompt_waits_for_matching_response_id() {
    let mut client = InMemoryWireClient::new();
    client
        .inject(WireMessage::SuccessResponse(
            crate::wire::protocol::JsonRpcSuccessResponse {
                jsonrpc: "2.0".to_string(),
                id: "req-999".to_string(),
                result: serde_json::json!({"status":"wrong"}),
            },
        ))
        .await;
    client
        .inject(WireMessage::Event(
            crate::wire::protocol::JsonRpcNotification {
                jsonrpc: "2.0".to_string(),
                method: "event".to_string(),
                params: crate::wire::protocol::EventParams {
                    event_type: "turn_begin".to_string(),
                    payload: serde_json::json!({"user_input":"hello"}),
                },
            },
        ))
        .await;
    client
        .inject(WireMessage::SuccessResponse(
            crate::wire::protocol::JsonRpcSuccessResponse {
                jsonrpc: "2.0".to_string(),
                id: "req-1".to_string(),
                result: serde_json::json!({"status":"ok"}),
            },
        ))
        .await;

    let result = client.prompt("hello").await.unwrap();
    assert_eq!(result.status, "ok");

    let buffered = client.read_message().await.unwrap();
    match buffered {
        WireMessage::SuccessResponse(resp) => assert_eq!(resp.id, "req-999"),
        other => panic!("expected buffered non-matching response, got {:?}", other),
    }
}

#[tokio::test]
async fn test_start_prompt_allows_streaming_before_response() {
    let mut client = InMemoryWireClient::new();
    client
        .inject(WireMessage::Event(
            crate::wire::protocol::JsonRpcNotification {
                jsonrpc: "2.0".to_string(),
                method: "event".to_string(),
                params: crate::wire::protocol::EventParams {
                    event_type: "turn_begin".to_string(),
                    payload: serde_json::json!({"user_input":"hello"}),
                },
            },
        ))
        .await;

    let id = client.start_prompt("hello").await.unwrap();
    assert_eq!(id, "req-1");

    let msg = client.read_message().await.unwrap();
    match msg {
        WireMessage::Event(ev) => assert_eq!(ev.params.event_type, "turn_begin"),
        other => panic!("expected streaming event, got {:?}", other),
    }
}

#[tokio::test]
async fn test_send_response_and_error() {
    let mut client = InMemoryWireClient::new();

    client
        .send_response("req-42", serde_json::json!({"ok": true}))
        .await
        .unwrap();

    client
        .send_error("req-43", -32600, "Invalid Request")
        .await
        .unwrap();

    let outgoing = client.outgoing().await;
    assert_eq!(outgoing.len(), 2);
}

#[tokio::test]
async fn test_process_messages_loop() {
    let mut client = InMemoryWireClient::new();
    client
        .inject(WireMessage::Event(
            crate::wire::protocol::JsonRpcNotification {
                jsonrpc: "2.0".to_string(),
                method: "event".to_string(),
                params: crate::wire::protocol::EventParams {
                    event_type: "turn_begin".to_string(),
                    payload: serde_json::json!({"user_input":"hello"}),
                },
            },
        ))
        .await;

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
}
