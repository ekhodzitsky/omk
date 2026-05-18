use omk::wire::protocol::WireMessage;
use serde_json::Value;
use std::fs;

fn golden_roundtrip(fixture_path: &str) {
    let raw = fs::read_to_string(fixture_path).unwrap_or_else(|e| {
        panic!("failed to read fixture {}: {}", fixture_path, e)
    });
    let msg: WireMessage = serde_json::from_str(&raw)
        .unwrap_or_else(|e| panic!("failed to deserialize {}: {}", fixture_path, e));
    let serialized =
        serde_json::to_string(&msg).unwrap_or_else(|e| panic!("failed to serialize: {}", e));
    let expected: Value = serde_json::from_str(&raw).expect("valid JSON");
    let actual: Value = serde_json::from_str(&serialized).expect("valid JSON");
    assert_eq!(
        actual, expected,
        "golden round-trip mismatch for {}",
        fixture_path
    );
}

#[test]
fn golden_initialize_request() {
    golden_roundtrip("tests/fixtures/wire/initialize_request.json");
}

#[test]
fn golden_initialize_response() {
    golden_roundtrip("tests/fixtures/wire/initialize_response.json");
}

#[test]
fn golden_event_notification() {
    golden_roundtrip("tests/fixtures/wire/event_notification.json");
}
