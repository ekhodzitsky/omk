pub(crate) fn value_as_string(value: &serde_json::Value) -> Option<String> {
    if let Some(text) = value.as_str() {
        return Some(text.to_string());
    }
    if let Some(number) = value.as_i64() {
        return Some(number.to_string());
    }
    if let Some(number) = value.as_u64() {
        return Some(number.to_string());
    }
    if let Some(number) = value.as_f64() {
        return Some(number.to_string());
    }
    if let Some(boolean) = value.as_bool() {
        return Some(boolean.to_string());
    }
    value.get("0")?.as_str().map(str::to_string)
}

pub(crate) fn gate_evidence_from_payload(payload: &serde_json::Value) -> Option<serde_json::Value> {
    let mut evidence = serde_json::Map::new();
    for key in [
        "command_line",
        "exit_code",
        "timed_out",
        "stdout_summary",
        "stderr_summary",
        "output_path",
        "timeout_secs",
    ] {
        if let Some(value) = payload.get(key) {
            evidence.insert(key.to_string(), value.clone());
        }
    }
    if evidence.is_empty() {
        None
    } else {
        Some(serde_json::Value::Object(evidence))
    }
}

pub(crate) fn gate_key_from_payload(payload: &serde_json::Value) -> Option<String> {
    payload
        .get("gate_id")
        .and_then(value_as_string)
        .or_else(|| payload.get("name").and_then(value_as_string))
}

pub(crate) fn copy_payload_field(
    payload: &serde_json::Value,
    into: &mut serde_json::Map<String, serde_json::Value>,
    key: &str,
) {
    if let Some(value) = payload.get(key) {
        into.insert(key.to_string(), value.clone());
    }
}
