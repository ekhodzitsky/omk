#![allow(dead_code)]

//! Configuration and XDG-compliant path resolution.
//!
//! Follows the XDG Base Directory Specification:
//! - Config:  $XDG_CONFIG_HOME/omk/  (~/.config/omk/)
//! - Data:    $XDG_DATA_HOME/omk/    (~/.local/share/omk/)
//! - State:   $XDG_STATE_HOME/omk/   (~/.local/state/omk/)
//! - Cache:   $XDG_CACHE_HOME/omk/   (~/.cache/omk/)
//!
//! Fallback for legacy: ~/.omk/ is symlinked/deprecated but still supported.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing::warn;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OmkConfig {
    /// Default number of workers for team mode
    #[serde(default = "default_team_size")]
    pub default_team_size: usize,

    /// Enable YOLO mode by default
    #[serde(default)]
    pub default_yolo: bool,

    /// Path to the Kimi CLI binary (auto-detected if empty)
    #[serde(default)]
    pub kimi_binary: Option<String>,

    /// Paths to additional skill directories
    #[serde(default)]
    pub extra_skill_dirs: Vec<PathBuf>,

    /// Telemetry: save metrics to state dir
    #[serde(default = "default_true")]
    pub enable_metrics: bool,

    /// External marketplace registries
    #[serde(default)]
    pub registries: Vec<String>,

    /// Notification webhook URLs
    #[serde(default)]
    pub webhooks: Option<crate::notifications::WebhookConfig>,
}

impl Default for OmkConfig {
    fn default() -> Self {
        Self {
            default_team_size: 2,
            default_yolo: false,
            kimi_binary: None,
            extra_skill_dirs: vec![],
            enable_metrics: true,
            registries: vec![],
            webhooks: None,
        }
    }
}

fn default_team_size() -> usize {
    2
}

fn default_true() -> bool {
    true
}

/// Resolve the XDG config directory for omk.
pub fn config_dir() -> PathBuf {
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        PathBuf::from(xdg).join("omk")
    } else {
        dirs::home_dir()
            .expect("No home directory")
            .join(".config")
            .join("omk")
    }
}

/// Resolve the XDG state directory for omk.
pub fn state_dir() -> PathBuf {
    if let Some(xdg) = std::env::var_os("XDG_STATE_HOME") {
        PathBuf::from(xdg).join("omk")
    } else {
        dirs::home_dir()
            .expect("No home directory")
            .join(".local")
            .join("state")
            .join("omk")
    }
}

/// Resolve the XDG data directory for omk.
pub fn data_dir() -> PathBuf {
    if let Some(xdg) = std::env::var_os("XDG_DATA_HOME") {
        PathBuf::from(xdg).join("omk")
    } else {
        dirs::home_dir()
            .expect("No home directory")
            .join(".local")
            .join("share")
            .join("omk")
    }
}

/// Resolve the XDG cache directory for omk.
pub fn cache_dir() -> PathBuf {
    if let Some(xdg) = std::env::var_os("XDG_CACHE_HOME") {
        PathBuf::from(xdg).join("omk")
    } else {
        dirs::home_dir()
            .expect("No home directory")
            .join(".cache")
            .join("omk")
    }
}

/// Legacy fallback: ~/.omk/
pub fn legacy_dir() -> PathBuf {
    dirs::home_dir()
        .expect("No home directory")
        .join(".omk")
}

/// Return the active state directory.
/// Prefers legacy ~/.omk/ if it exists, otherwise uses XDG state dir.
pub fn omk_state_dir() -> PathBuf {
    let legacy = legacy_dir();
    if legacy.exists() {
        legacy.join("state")
    } else {
        state_dir()
    }
}

/// Return the active data directory.
/// Prefers legacy ~/.omk/ if it exists, otherwise uses XDG data dir.
pub fn omk_data_dir() -> PathBuf {
    let legacy = legacy_dir();
    if legacy.exists() {
        legacy
    } else {
        data_dir()
    }
}

/// Load config from disk or return defaults.
pub async fn load_config() -> Result<OmkConfig> {
    let path = config_dir().join("config.toml");

    if !path.exists() {
        // Check legacy location for migration
        let legacy = legacy_dir().join("config.toml");
        if legacy.exists() {
            warn!(legacy = %legacy.display(), "Using legacy config location. Consider migrating to XDG dirs.");
            let content = tokio::fs::read_to_string(&legacy).await?;
            return parse_config(&content);
        }
        return Ok(OmkConfig::default());
    }

    let content = tokio::fs::read_to_string(&path).await?;
    parse_config(&content)
}

fn parse_config(content: &str) -> Result<OmkConfig> {
    let config: OmkConfig = toml::from_str(content).context("Failed to parse config.toml")?;
    Ok(config)
}

/// Initialize XDG directories.
pub async fn ensure_dirs() -> Result<()> {
    tokio::fs::create_dir_all(config_dir()).await?;
    tokio::fs::create_dir_all(state_dir()).await?;
    tokio::fs::create_dir_all(data_dir()).await?;
    tokio::fs::create_dir_all(cache_dir()).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xdg_paths_exist() {
        // Just verify they don't panic
        let _ = config_dir();
        let _ = state_dir();
        let _ = data_dir();
        let _ = cache_dir();
    }

    #[test]
    fn test_parse_default_config() {
        let config = parse_config("").unwrap();
        assert_eq!(config.default_team_size, 2);
        assert!(!config.default_yolo);
        assert!(config.enable_metrics);
        assert!(config.registries.is_empty());
    }

    #[test]
    fn test_parse_custom_config() {
        let config = parse_config(
            r#"
default_team_size = 5
default_yolo = true
enable_metrics = false
kimi_binary = "/opt/kimi"
"#,
        )
        .unwrap();
        assert_eq!(config.default_team_size, 5);
        assert!(config.default_yolo);
        assert!(!config.enable_metrics);
        assert_eq!(config.kimi_binary, Some("/opt/kimi".to_string()));
    }
}
