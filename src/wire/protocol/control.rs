use serde::{Deserialize, Serialize};
use serde_json::Value;

// ============================================================================
// Replay
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ReplayParams {}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReplayResult {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub events: Option<Vec<Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requests: Option<Vec<Value>>,
}

// ============================================================================
// Steer
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SteerParams {
    pub user_input: UserInput,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SteerResult {
    pub status: String,
}

// ============================================================================
// SetPlanMode
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SetPlanModeParams {
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SetPlanModeResult {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan_mode: Option<bool>,
}

// ============================================================================
// Cancel
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct CancelParams {}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct CancelResult {}

use crate::wire::protocol::UserInput;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cancel_params_result() {
        let params = CancelParams {};
        let json = serde_json::to_string(&params).unwrap();
        assert_eq!(json, "{}");
        let de: CancelParams = serde_json::from_str(&json).unwrap();
        assert_eq!(de, params);

        let result = CancelResult {};
        let json = serde_json::to_string(&result).unwrap();
        assert_eq!(json, "{}");
        let de: CancelResult = serde_json::from_str(&json).unwrap();
        assert_eq!(de, result);
    }

    #[test]
    fn test_set_plan_mode_params_result() {
        let params = SetPlanModeParams { enabled: true };
        let json = serde_json::to_value(&params).unwrap();
        assert_eq!(json["enabled"], true);

        let result = SetPlanModeResult {
            status: "ok".to_string(),
            plan_mode: Some(true),
        };
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["status"], "ok");
        assert_eq!(json["plan_mode"], true);
    }

    #[test]
    fn test_steer_params_result() {
        use crate::wire::protocol::UserInput;
        let params = SteerParams {
            user_input: UserInput::Text("use rust".to_string()),
        };
        let json = serde_json::to_value(&params).unwrap();
        assert_eq!(json["user_input"], "use rust");

        let result = SteerResult {
            status: "steered".to_string(),
        };
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["status"], "steered");
    }

    #[test]
    fn test_replay_params_result() {
        let params = ReplayParams {};
        let json = serde_json::to_string(&params).unwrap();
        assert_eq!(json, "{}");

        let result = ReplayResult {
            status: "finished".to_string(),
            events: None,
            requests: None,
        };
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["status"], "finished");
        assert!(!json.as_object().unwrap().contains_key("events"));
    }
}
