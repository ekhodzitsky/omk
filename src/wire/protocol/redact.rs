use once_cell::sync::Lazy;
use regex::Regex;
use serde_json::Value;
use std::borrow::Cow;

pub(crate) const REDACTED_SECRET: &str = "[REDACTED]";

/// Best-effort value-shape redaction patterns.
///
/// Key-based redaction in [`is_sensitive_wire_key`] is the primary defense:
/// agents control payload structure and almost always store secrets behind a
/// well-known key. The patterns below are defense in depth for values that
/// leak through nested or unstructured fields — for example a model
/// transcript that quotes an environment variable verbatim. They are
/// intentionally conservative (anchored, long minimum lengths) so legitimate
/// content does not get false-positive scrubbed.
static SECRET_VALUE_PATTERNS: Lazy<std::result::Result<Vec<Regex>, regex::Error>> =
    Lazy::new(|| {
        let patterns: &[&str] = &[
            // GitHub personal-access / OAuth / refresh tokens.
            r"\bgh[pousr]_[A-Za-z0-9]{20,}\b",
            // AWS access key id.
            r"\bAKIA[0-9A-Z]{16}\b",
            // Slack bot/user/app/refresh tokens (xoxb-, xoxa-, xoxp-, xoxr-, xoxs-).
            r"\bxox[abprs]-[A-Za-z0-9-]{10,}\b",
            // Stripe live/test secret keys.
            r"\bsk_(?:live|test)_[A-Za-z0-9]{16,}\b",
            // Generic Bearer-token-shaped fragments that survived key redaction.
            r"(?i)\bBearer\s+[A-Za-z0-9._~+/=-]{20,}\b",
            // PEM private key block markers (one finding flags the whole block).
            r"-----BEGIN [A-Z ]*PRIVATE KEY-----",
        ];
        patterns.iter().map(|&p| Regex::new(p)).collect()
    });

pub fn scrub_secret_patterns(input: &str) -> Cow<'_, str> {
    let patterns = match SECRET_VALUE_PATTERNS.as_ref() {
        Ok(p) => p,
        Err(_) => return Cow::Borrowed(input),
    };
    let mut current: Cow<'_, str> = Cow::Borrowed(input);
    for re in patterns.iter() {
        match re.replace_all(current.as_ref(), REDACTED_SECRET) {
            Cow::Borrowed(_) => {}
            Cow::Owned(new) => current = Cow::Owned(new),
        }
    }
    current
}

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
        Value::String(s) => match scrub_secret_patterns(s) {
            Cow::Borrowed(_) => value.clone(),
            Cow::Owned(scrubbed) => Value::String(scrubbed),
        },
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

    #[test]
    fn redact_value_patterns_scrubs_known_token_shapes() {
        // Each pattern is exercised exactly once, with a surrounding string
        // that proves only the secret-shaped fragment is replaced — adjacent
        // narrative text must be preserved so reviewers can still read the
        // log entry.
        //
        // Each fixture is assembled at runtime from harmless fragments so the
        // source file never contains a contiguous string that matches a real
        // GitHub / AWS / Slack / Stripe token signature. This is what keeps
        // GitHub push-protection from rejecting these tests as leaked
        // credentials.
        let github_pat = ["ghp", "_", "abcdefghijklmnop1234567890abcdef0011"].concat();
        let aws_key = ["AKIA", "ABCDEFGHIJKLMNOP"].concat();
        let slack_token = ["xoxb", "-", "1234567890-abcdefghij1"].concat();
        let stripe_key = ["sk_live", "_", "abcdefghij1234567890ABCD"].concat();
        let bearer = ["Bearer", " ", "abcdef0123456789abcdef0123456789"].concat();
        let pem = "-----BEGIN RSA PRIVATE KEY-----".to_string();

        let raw = json!({
            "transcript": [
                format!("leaked github pat {github_pat} in env"),
                format!("old aws key was {aws_key} and is rotated"),
                format!("slack hook {slack_token} expired"),
                format!("stripe payload {stripe_key} used in tests"),
                format!("header value: Authorization: {bearer}"),
                format!("pem block {pem} payload"),
            ]
        });
        let redacted = redact_wire_secrets(&raw);
        let transcript = redacted["transcript"].as_array().unwrap();

        assert_eq!(transcript[0], "leaked github pat [REDACTED] in env");
        assert_eq!(transcript[1], "old aws key was [REDACTED] and is rotated");
        assert_eq!(transcript[2], "slack hook [REDACTED] expired");
        assert_eq!(transcript[3], "stripe payload [REDACTED] used in tests");
        assert_eq!(transcript[4], "header value: Authorization: [REDACTED]");
        assert_eq!(transcript[5], "pem block [REDACTED] payload");
    }

    #[test]
    fn redact_value_patterns_preserves_benign_strings() {
        // Strings that merely mention secret-related concepts but contain no
        // secret-shaped fragments must round-trip unchanged. This guards
        // against over-eager pattern broadening down the line.
        let raw = json!({
            "summary": "rotated the github token quarterly",
            "url": "https://docs.example.com/auth/api-key.html",
            "code": "let token = std::env::var(\"GITHUB_TOKEN\");"
        });
        let redacted = redact_wire_secrets(&raw);

        assert_eq!(redacted["summary"], "rotated the github token quarterly");
        assert_eq!(
            redacted["url"],
            "https://docs.example.com/auth/api-key.html"
        );
        assert_eq!(
            redacted["code"],
            "let token = std::env::var(\"GITHUB_TOKEN\");"
        );
    }

    #[test]
    fn redact_value_patterns_are_idempotent() {
        // Re-running redaction on its own output must not double-encode the
        // sentinel — downstream consumers reuse the helper when merging
        // archived logs, so stability matters. The fixture is assembled at
        // runtime to keep GitHub push-protection from misclassifying the
        // source as a leaked credential.
        let github_pat = ["ghp", "_", "abcdefghijklmnop1234567890abcdef0011"].concat();
        let once = redact_wire_secrets(&json!({
            "msg": github_pat,
        }));
        let twice = redact_wire_secrets(&once);
        assert_eq!(once, twice);
        assert_eq!(once["msg"], REDACTED_SECRET);
    }
}
