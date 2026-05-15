use serde_json::Value;

use super::types::{Event, EventParams};

// ============================================================================
// Conversions
// ============================================================================

impl Event {
    /// Convert a typed event into raw wire params.
    pub fn to_params(&self) -> Result<EventParams, serde_json::Error> {
        let mut value = serde_json::to_value(self)?;
        let event_type = if let Value::Object(ref mut map) = value {
            map.remove("type")
                .and_then(|v| v.as_str().map(String::from))
                .unwrap_or_default()
        } else {
            String::new()
        };
        Ok(EventParams {
            event_type,
            payload: value,
        })
    }
}

impl EventParams {
    /// Return a stable snake_case event type for matching across Kimi releases.
    pub fn normalized_event_type(&self) -> String {
        normalize_wire_kind(&self.event_type)
    }

    /// Convert raw wire params into a typed event.
    pub fn to_event(&self) -> Result<Event, serde_json::Error> {
        let mut value = self.payload.clone();
        if let Value::Object(ref mut map) = value {
            map.insert(
                "type".to_string(),
                Value::String(self.normalized_event_type()),
            );
        }
        serde_json::from_value(value)
    }
}

fn normalize_wire_kind(raw: &str) -> String {
    let mut normalized = String::with_capacity(raw.len() + 4);
    let mut previous_was_separator = true;

    for ch in raw.chars() {
        if ch == '-' || ch == '_' || ch == ' ' {
            if !normalized.ends_with('_') {
                normalized.push('_');
            }
            previous_was_separator = true;
            continue;
        }

        if ch.is_ascii_uppercase() {
            if !previous_was_separator && !normalized.ends_with('_') {
                normalized.push('_');
            }
            normalized.push(ch.to_ascii_lowercase());
            previous_was_separator = false;
        } else {
            normalized.push(ch.to_ascii_lowercase());
            previous_was_separator = false;
        }
    }

    normalized.trim_matches('_').to_string()
}
