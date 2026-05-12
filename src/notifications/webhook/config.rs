use serde::{Deserialize, Serialize};

/// Supported webhook destinations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookConfig {
    pub discord: Option<String>,
    pub slack: Option<String>,
    pub telegram: Option<String>,
}
