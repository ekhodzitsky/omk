use serde_json::Value;

pub(crate) const REDACTED_SECRET: &str = "[REDACTED]";

/// Redact sensitive fields in wire JSON payloads while preserving structure.
///
/// This is used before writing durable wire logs/evidence.
pub fn redact_wire_secrets(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut redacted = serde_json::Map::with_capacity(map.len());
            for (key, entry) in map {
                if is_sensitive_wire_key(key) {
                    redacted.insert(key.clone(), Value::String(REDACTED_SECRET.to_string()));
                } else {
                    redacted.insert(key.clone(), redact_wire_secrets(entry));
                }
            }
            Value::Object(redacted)
        }
        Value::Array(items) => Value::Array(items.iter().map(redact_wire_secrets).collect()),
        _ => value.clone(),
    }
}

fn is_sensitive_wire_key(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    matches!(
        lower.as_str(),
        "api_key" | "apikey" | "token" | "authorization" | "password" | "secret"
    ) || lower.ends_with("_token")
        || lower.ends_with("-token")
        || lower.ends_with("_secret")
        || lower.ends_with("-secret")
        || lower.contains("authorization")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_redact_wire_secrets_recursive() {
        let raw = json!({
            "api_key": "abc123",
            "nested": {
                "token": "tok123",
                "headers": {
                    "authorization": "Bearer abc"
                },
                "token_usage": 42
            },
            "items": [
                {"password": "pass1"},
                {"safe": "value"}
            ]
        });
        let redacted = redact_wire_secrets(&raw);

        assert_eq!(redacted["api_key"], REDACTED_SECRET);
        assert_eq!(redacted["nested"]["token"], REDACTED_SECRET);
        assert_eq!(
            redacted["nested"]["headers"]["authorization"],
            REDACTED_SECRET
        );
        assert_eq!(redacted["nested"]["token_usage"], 42);
        assert_eq!(redacted["items"][0]["password"], REDACTED_SECRET);
        assert_eq!(redacted["items"][1]["safe"], "value");
    }

    #[test]
    fn test_redact_wire_secrets_case_insensitive_and_suffix() {
        let raw = json!({
            "Authorization": "Bearer 1",
            "access_token": "tok_1",
            "client_secret": "secret_1",
            "ApiKey": "key_1",
            "safe_value": "ok"
        });
        let redacted = redact_wire_secrets(&raw);

        assert_eq!(redacted["Authorization"], REDACTED_SECRET);
        assert_eq!(redacted["access_token"], REDACTED_SECRET);
        assert_eq!(redacted["client_secret"], REDACTED_SECRET);
        assert_eq!(redacted["ApiKey"], REDACTED_SECRET);
        assert_eq!(redacted["safe_value"], "ok");
    }
}
