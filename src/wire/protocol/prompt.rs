use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PromptParams {
    pub user_input: UserInput,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum UserInput {
    Text(String),
    Parts(Vec<ContentPart>),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PromptResult {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub steps: Option<PromptSteps>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum PromptSteps {
    Count(u64),
    LegacyTrace(Vec<Value>),
}

use crate::wire::protocol::ContentPart;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::protocol::{TextPart, ThinkPart};

    #[test]
    fn test_prompt_params_text() {
        let params = PromptParams {
            user_input: UserInput::Text("hello world".to_string()),
        };
        let json = serde_json::to_value(&params).unwrap();
        assert_eq!(json["user_input"], "hello world");
        let de: PromptParams = serde_json::from_value(json).unwrap();
        assert_eq!(de, params);
    }

    #[test]
    fn test_prompt_params_parts() {
        let params = PromptParams {
            user_input: UserInput::Parts(vec![
                ContentPart::Text(TextPart {
                    text: "hello".to_string(),
                }),
                ContentPart::Think(ThinkPart {
                    think: "thinking...".to_string(),
                    encrypted: Some(true),
                }),
            ]),
        };
        let json = serde_json::to_value(&params).unwrap();
        let parts = json["user_input"].as_array().unwrap();
        assert_eq!(parts[0]["type"], "text");
        assert_eq!(parts[0]["text"], "hello");
        assert_eq!(parts[1]["type"], "think");
        assert_eq!(parts[1]["think"], "thinking...");
        assert_eq!(parts[1]["encrypted"], true);
        let de: PromptParams = serde_json::from_value(json).unwrap();
        assert_eq!(de, params);
    }
}
