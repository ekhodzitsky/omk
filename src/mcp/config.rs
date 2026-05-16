use crate::error::OmkError;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct McpConfig {
    #[serde(default)]
    pub servers: HashMap<String, McpServerConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct McpServerConfig {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
}

impl McpConfig {
    pub async fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let contents = tokio::fs::read_to_string(path)
            .await
            .with_context(|| format!("failed to read MCP config from {}", path.display()))?;
        let config: McpConfig =
            serde_json::from_str(&contents).map_err(|e| OmkError::McpConfig {
                path: path.to_path_buf(),
                reason: e.to_string(),
            })?;
        Ok(config)
    }
    pub fn default_path() -> PathBuf {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let local = cwd.join(".omk").join("mcp.json");
        if local.exists() {
            return local;
        }
        dirs::config_dir()
            .map(|d| d.join("omk").join("mcp.json"))
            .unwrap_or(local)
    }
}
