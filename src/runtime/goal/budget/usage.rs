use std::collections::HashMap;
use std::path::Path;

use crate::runtime::goal::state::{GoalState, GOAL_AGENT_RUNS_DIR, GOAL_ARTIFACTS_DIR};

const STANDARD_INPUT_USD_PER_1M_TOKENS: f64 = 2.0;
const STANDARD_OUTPUT_USD_PER_1M_TOKENS: f64 = 8.0;

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct GoalBudgetUsage {
    pub used_tokens: u64,
    pub estimated_cost_usd: f64,
}

pub(super) async fn collect_goal_budget_usage(state: &GoalState) -> GoalBudgetUsage {
    let root = state
        .state_dir
        .join(GOAL_ARTIFACTS_DIR)
        .join(GOAL_AGENT_RUNS_DIR);
    if !root.exists() {
        return GoalBudgetUsage::default();
    }

    let mut usage = GoalBudgetUsage::default();
    for entry in walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_file())
        .filter(|entry| entry.file_name() == "wire-events.jsonl")
    {
        usage.add(collect_wire_event_file_usage(entry.path()).await);
    }
    usage
}

async fn collect_wire_event_file_usage(path: &Path) -> GoalBudgetUsage {
    let Ok(content) = tokio::fs::read_to_string(path).await else {
        return GoalBudgetUsage::default();
    };

    let mut anonymous = GoalBudgetUsage::default();
    let mut by_message: HashMap<String, GoalBudgetUsage> = HashMap::new();
    for line in content.lines().filter(|line| !line.trim().is_empty()) {
        let Some((message_id, usage)) = parse_wire_status_usage(line) else {
            continue;
        };
        if let Some(message_id) = message_id {
            by_message
                .entry(message_id)
                .and_modify(|current| current.keep_max(usage))
                .or_insert(usage);
        } else {
            anonymous.add(usage);
        }
    }

    for usage in by_message.into_values() {
        anonymous.add(usage);
    }
    anonymous
}

fn parse_wire_status_usage(line: &str) -> Option<(Option<String>, GoalBudgetUsage)> {
    let value: serde_json::Value = serde_json::from_str(line).ok()?;
    let params = value.get("params")?;
    let event_type = params.get("type")?.as_str()?;
    if !event_type.eq_ignore_ascii_case("status_update")
        && !event_type.eq_ignore_ascii_case("status-update")
    {
        return None;
    }
    let payload = params.get("payload")?;
    let message_id = payload
        .get("message_id")
        .and_then(|value| value.as_str())
        .map(ToString::to_string);
    token_usage_from_payload(payload).map(|usage| (message_id, usage))
}

fn token_usage_from_payload(payload: &serde_json::Value) -> Option<GoalBudgetUsage> {
    let token_usage = payload.get("token_usage")?;
    if let Some(total) = token_usage.as_u64() {
        return Some(GoalBudgetUsage {
            used_tokens: total,
            estimated_cost_usd: estimate_unknown_token_cost(total),
        });
    }

    let input_other = token_usage
        .get("input_other")
        .and_then(|value| value.as_u64())
        .unwrap_or(0);
    let input_cache_read = token_usage
        .get("input_cache_read")
        .and_then(|value| value.as_u64())
        .unwrap_or(0);
    let input_cache_creation = token_usage
        .get("input_cache_creation")
        .and_then(|value| value.as_u64())
        .unwrap_or(0);
    let output = token_usage
        .get("output")
        .and_then(|value| value.as_u64())
        .unwrap_or(0);
    let input = input_other
        .saturating_add(input_cache_read)
        .saturating_add(input_cache_creation);
    let total = input.saturating_add(output);
    (total > 0).then(|| GoalBudgetUsage {
        used_tokens: total,
        estimated_cost_usd: estimate_split_token_cost(input, output),
    })
}

fn estimate_unknown_token_cost(tokens: u64) -> f64 {
    (tokens as f64 / 1_000_000.0) * STANDARD_OUTPUT_USD_PER_1M_TOKENS
}

fn estimate_split_token_cost(input_tokens: u64, output_tokens: u64) -> f64 {
    (input_tokens as f64 / 1_000_000.0) * STANDARD_INPUT_USD_PER_1M_TOKENS
        + (output_tokens as f64 / 1_000_000.0) * STANDARD_OUTPUT_USD_PER_1M_TOKENS
}

impl GoalBudgetUsage {
    pub(super) fn add(&mut self, other: GoalBudgetUsage) {
        self.used_tokens = self.used_tokens.saturating_add(other.used_tokens);
        self.estimated_cost_usd += other.estimated_cost_usd;
    }

    pub(super) fn keep_max(&mut self, other: GoalBudgetUsage) {
        if other.used_tokens > self.used_tokens {
            *self = other;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FLOAT_EPSILON: f64 = 1e-10;

    #[test]
    fn test_token_usage_from_payload_flat_total() {
        let payload = serde_json::json!({"token_usage": 1_500_000});
        let usage = token_usage_from_payload(&payload).unwrap();
        assert_eq!(usage.used_tokens, 1_500_000);
        assert!(
            (usage.estimated_cost_usd - 12.0).abs() < FLOAT_EPSILON,
            "expected 12.0, got {}",
            usage.estimated_cost_usd
        );
    }

    #[test]
    fn test_token_usage_from_payload_split_fields() {
        let payload = serde_json::json!({
            "token_usage": {
                "input_other": 500_000,
                "input_cache_read": 200_000,
                "input_cache_creation": 100_000,
                "output": 200_000,
            }
        });
        let usage = token_usage_from_payload(&payload).unwrap();
        assert_eq!(usage.used_tokens, 1_000_000);
        let expected_cost = (800_000.0 / 1_000_000.0) * 2.0 + (200_000.0 / 1_000_000.0) * 8.0;
        assert!(
            (usage.estimated_cost_usd - expected_cost).abs() < FLOAT_EPSILON,
            "expected {}, got {}",
            expected_cost,
            usage.estimated_cost_usd
        );
    }

    #[test]
    fn test_token_usage_from_payload_missing_returns_none() {
        let payload = serde_json::json!({"status": "ok"});
        assert!(token_usage_from_payload(&payload).is_none());
    }

    #[test]
    fn test_token_usage_from_payload_zero_total_returns_none() {
        let payload = serde_json::json!({
            "token_usage": {
                "input_other": 0,
                "output": 0,
            }
        });
        assert!(token_usage_from_payload(&payload).is_none());
    }

    #[test]
    fn test_parse_wire_status_usage_status_update() {
        let line = r#"{"params":{"type":"status_update","payload":{"message_id":"msg-1","token_usage":500000}}}"#;
        let (msg_id, usage) = parse_wire_status_usage(line).unwrap();
        assert_eq!(msg_id, Some("msg-1".to_string()));
        assert_eq!(usage.used_tokens, 500_000);
    }

    #[test]
    fn test_parse_wire_status_usage_status_update_kebab_case() {
        let line = r#"{"params":{"type":"status-update","payload":{"message_id":"msg-2","token_usage":750000}}}"#;
        let (msg_id, usage) = parse_wire_status_usage(line).unwrap();
        assert_eq!(msg_id, Some("msg-2".to_string()));
        assert_eq!(usage.used_tokens, 750_000);
    }

    #[test]
    fn test_parse_wire_status_usage_ignores_other_events() {
        let line = r#"{"params":{"type":"text_chunk","payload":{"text":"hello"}}}"#;
        assert!(parse_wire_status_usage(line).is_none());
    }

    #[test]
    fn test_estimate_unknown_token_cost() {
        assert!((estimate_unknown_token_cost(1_000_000) - 8.0).abs() < FLOAT_EPSILON);
        assert!((estimate_unknown_token_cost(500_000) - 4.0).abs() < FLOAT_EPSILON);
    }

    #[test]
    fn test_estimate_split_token_cost() {
        assert!((estimate_split_token_cost(1_000_000, 0) - 2.0).abs() < FLOAT_EPSILON);
        assert!((estimate_split_token_cost(0, 1_000_000) - 8.0).abs() < FLOAT_EPSILON);
        assert!((estimate_split_token_cost(1_000_000, 1_000_000) - 10.0).abs() < FLOAT_EPSILON);
    }

    #[test]
    fn test_goal_budget_usage_add() {
        let mut a = GoalBudgetUsage {
            used_tokens: 100,
            estimated_cost_usd: 1.5,
        };
        let b = GoalBudgetUsage {
            used_tokens: 200,
            estimated_cost_usd: 2.5,
        };
        a.add(b);
        assert_eq!(a.used_tokens, 300);
        assert!((a.estimated_cost_usd - 4.0).abs() < FLOAT_EPSILON);
    }

    #[test]
    fn test_goal_budget_usage_keep_max() {
        let mut a = GoalBudgetUsage {
            used_tokens: 100,
            estimated_cost_usd: 1.0,
        };
        let b = GoalBudgetUsage {
            used_tokens: 200,
            estimated_cost_usd: 2.0,
        };
        a.keep_max(b);
        assert_eq!(a.used_tokens, 200);
        assert!((a.estimated_cost_usd - 2.0).abs() < FLOAT_EPSILON);
    }

    #[test]
    fn test_goal_budget_usage_keep_max_smaller_ignored() {
        let mut a = GoalBudgetUsage {
            used_tokens: 300,
            estimated_cost_usd: 3.0,
        };
        let b = GoalBudgetUsage {
            used_tokens: 200,
            estimated_cost_usd: 2.0,
        };
        a.keep_max(b);
        assert_eq!(a.used_tokens, 300);
        assert!((a.estimated_cost_usd - 3.0).abs() < FLOAT_EPSILON);
    }
}
