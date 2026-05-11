use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InitializeParams {
    pub protocol_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client: Option<ClientInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_tools: Option<Vec<Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<ClientCapabilities>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hooks: Option<Vec<WireHookSubscription>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClientInfo {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClientCapabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supports_question: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supports_plan_mode: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WireHookSubscription {
    pub id: String,
    pub event: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matcher: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InitializeResult {
    pub protocol_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slash_commands: Option<Vec<Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_tools: Option<Vec<Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hooks: Option<Value>,
}
