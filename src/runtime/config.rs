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
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::warn;

use crate::runtime::gates::circuit_breaker::CircuitBreakerConfig;
use crate::runtime::scheduler::pool::AgentPool;
use crate::runtime::wire_worker::ApprovalPolicy;

/// Directory name for team state.
pub const TEAM_DIR: &str = "team";
/// Directory name for worker state within a team/run.
pub const WORKERS_DIR: &str = "workers";
/// File name for the append-only event log.
pub const EVENTS_FILE: &str = "events.jsonl";
/// Legacy alias for the append-only event log (read-only fallback).
pub const EVENTS_FILE_ALIAS: &str = "event-log.jsonl";
/// File name for worker heartbeat JSON.
pub const HEARTBEAT_FILE: &str = "heartbeat.json";
/// File name for worker inbox JSONL.
pub const INBOX_FILE: &str = "inbox.jsonl";
/// File name for worker outbox JSONL.
pub const OUTBOX_FILE: &str = "outbox.jsonl";

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

    /// Default approval policy for wire workers
    #[serde(default)]
    pub approval_policy: ApprovalPolicy,

    /// Default approval timeout in seconds
    #[serde(default = "default_approval_timeout_secs")]
    pub approval_timeout_secs: u64,

    /// Agent pool configurations keyed by pool name.
    #[serde(default)]
    pub pools: HashMap<String, AgentPool>,

    /// Global circuit breaker defaults for verification gates.
    #[serde(default)]
    pub circuit_breaker: Option<CircuitBreakerConfig>,

    /// Stagnation recovery configuration
    #[serde(default)]
    pub stagnation: StagnationConfig,
}

fn default_approval_timeout_secs() -> u64 {
    300
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
            approval_policy: ApprovalPolicy::default(),
            approval_timeout_secs: default_approval_timeout_secs(),
            pools: HashMap::new(),
            circuit_breaker: None,
            stagnation: StagnationConfig::default(),
        }
    }
}

fn default_team_size() -> usize {
    2
}

fn default_true() -> bool {
    true
}

/// Configuration for adaptive stagnation recovery.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StagnationConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_window_size")]
    pub window_size: usize,
    #[serde(default = "default_min_stagnant_metrics")]
    pub min_stagnant_metrics: usize,
    #[serde(default = "default_warmup_iterations")]
    pub warmup_iterations: usize,
    #[serde(default)]
    pub auto_recover: bool,
    #[serde(default = "default_max_recoveries_per_goal")]
    pub max_recoveries_per_goal: u32,
    #[serde(default)]
    pub thresholds: StagnationThresholdsConfig,
}

impl Default for StagnationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            window_size: 5,
            min_stagnant_metrics: 3,
            warmup_iterations: 3,
            auto_recover: false,
            max_recoveries_per_goal: 3,
            thresholds: StagnationThresholdsConfig::default(),
        }
    }
}

fn default_window_size() -> usize {
    5
}

fn default_min_stagnant_metrics() -> usize {
    3
}

fn default_warmup_iterations() -> usize {
    3
}

fn default_max_recoveries_per_goal() -> u32 {
    3
}

/// Threshold configuration for stagnation detection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StagnationThresholdsConfig {
    #[serde(default = "default_proof_score_epsilon")]
    pub proof_score_epsilon: f64,
    #[serde(default = "default_commit_velocity_min")]
    pub commit_velocity_min: u32,
    #[serde(default = "default_gate_pass_rate_epsilon")]
    pub gate_pass_rate_epsilon: f64,
    #[serde(default = "default_coverage_epsilon")]
    pub coverage_epsilon: f64,
    #[serde(default = "default_token_efficiency_max")]
    pub token_efficiency_max: f64,
    #[serde(default = "default_file_churn_max")]
    pub file_churn_max: u32,
}

impl Default for StagnationThresholdsConfig {
    fn default() -> Self {
        Self {
            proof_score_epsilon: 0.01,
            commit_velocity_min: 1,
            gate_pass_rate_epsilon: 0.05,
            coverage_epsilon: 0.01,
            token_efficiency_max: 1000.0,
            file_churn_max: 10,
        }
    }
}

fn default_proof_score_epsilon() -> f64 {
    0.01
}

fn default_commit_velocity_min() -> u32 {
    1
}

fn default_gate_pass_rate_epsilon() -> f64 {
    0.05
}

fn default_coverage_epsilon() -> f64 {
    0.01
}

fn default_token_efficiency_max() -> f64 {
    1000.0
}

fn default_file_churn_max() -> u32 {
    10
}

fn home_dir() -> anyhow::Result<PathBuf> {
    dirs::home_dir().ok_or_else(|| anyhow::anyhow!("HOME directory not found"))
}

/// Resolve the XDG config directory for omk.
pub fn config_dir() -> PathBuf {
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        PathBuf::from(xdg).join("omk")
    } else {
        home_dir()
            .map(|h| h.join(".config").join("omk"))
            .unwrap_or_else(|_| PathBuf::from("/tmp/.config/omk"))
    }
}

/// Resolve the XDG state directory for omk.
pub fn state_dir() -> PathBuf {
    if let Some(xdg) = std::env::var_os("XDG_STATE_HOME") {
        PathBuf::from(xdg).join("omk")
    } else {
        home_dir()
            .map(|h| h.join(".local").join("state").join("omk"))
            .unwrap_or_else(|_| PathBuf::from("/tmp/.local/state/omk"))
    }
}

/// Resolve the XDG data directory for omk.
pub fn data_dir() -> PathBuf {
    if let Some(xdg) = std::env::var_os("XDG_DATA_HOME") {
        PathBuf::from(xdg).join("omk")
    } else {
        home_dir()
            .map(|h| h.join(".local").join("share").join("omk"))
            .unwrap_or_else(|_| PathBuf::from("/tmp/.local/share/omk"))
    }
}

/// Resolve the XDG cache directory for omk.
pub fn cache_dir() -> PathBuf {
    if let Some(xdg) = std::env::var_os("XDG_CACHE_HOME") {
        PathBuf::from(xdg).join("omk")
    } else {
        home_dir()
            .map(|h| h.join(".cache").join("omk"))
            .unwrap_or_else(|_| PathBuf::from("/tmp/.cache/omk"))
    }
}

/// Legacy fallback: ~/.omk/
pub fn legacy_dir() -> PathBuf {
    home_dir()
        .map(|h| h.join(".omk"))
        .unwrap_or_else(|_| PathBuf::from("/tmp/.omk"))
}

/// Resolve the event log path for readers.
///
/// Prefers the canonical `events.jsonl` and falls back to the legacy
/// `event-log.jsonl` alias when the canonical file is absent.
pub fn resolve_event_log_for_read(state_dir: &Path) -> PathBuf {
    let canonical = state_dir.join(EVENTS_FILE);
    if canonical.exists() {
        return canonical;
    }

    let legacy_alias = state_dir.join(EVENTS_FILE_ALIAS);
    if legacy_alias.exists() {
        return legacy_alias;
    }

    canonical
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
    ensure_private_dir(&config_dir()).await?;
    ensure_private_dir(&state_dir()).await?;
    ensure_private_dir(&data_dir()).await?;
    ensure_private_dir(&cache_dir()).await?;
    Ok(())
}

/// Create a directory and make it readable, writable, and traversable only
/// by the current user on Unix platforms.
pub async fn ensure_private_dir(path: &Path) -> Result<()> {
    tokio::fs::create_dir_all(path)
        .await
        .with_context(|| format!("Failed to create directory: {}", path.display()))?;
    set_private_dir_permissions(path).await
}

#[cfg(unix)]
async fn set_private_dir_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    tokio::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
        .await
        .with_context(|| format!("Failed to harden directory permissions: {}", path.display()))?;
    Ok(())
}

#[cfg(not(unix))]
async fn set_private_dir_permissions(_path: &Path) -> Result<()> {
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
        assert_eq!(config.approval_timeout_secs, 300);
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

    #[test]
    fn test_resolve_event_log_for_read_prefers_canonical_file() {
        let temp = tempfile::tempdir().unwrap();
        let state_dir = temp.path();
        let canonical = state_dir.join(EVENTS_FILE);
        let alias = state_dir.join(EVENTS_FILE_ALIAS);

        std::fs::write(&canonical, "{}\n").unwrap();
        std::fs::write(&alias, "{}\n").unwrap();

        assert_eq!(resolve_event_log_for_read(state_dir), canonical);
    }

    #[test]
    fn test_resolve_event_log_for_read_falls_back_to_alias() {
        let temp = tempfile::tempdir().unwrap();
        let state_dir = temp.path();
        let alias = state_dir.join(EVENTS_FILE_ALIAS);

        std::fs::write(&alias, "{}\n").unwrap();

        assert_eq!(resolve_event_log_for_read(state_dir), alias);
    }
}
