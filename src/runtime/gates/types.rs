use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::runtime::gates::circuit_breaker::CircuitBreakerConfig;

pub(super) const SKIPPED_GATE_COMMAND: &str = "__omk_internal_skipped_gate__";

fn default_required() -> bool {
    true
}

/// A single verification gate definition.
#[derive(Debug, Clone, Serialize)]
pub struct GateDef {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    #[serde(default = "default_required")]
    pub required: bool,
    #[serde(default)]
    pub timeout_secs: u64,
    #[serde(default)]
    pub circuit_breaker: Option<CircuitBreakerConfig>,
}

#[derive(Debug, Deserialize)]
struct GateDefConfig {
    name: String,
    command: String,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default = "default_required")]
    required: bool,
    #[serde(default)]
    timeout_secs: u64,
    #[serde(default, alias = "allow-fail")]
    allow_fail: bool,
    #[serde(default, alias = "skip", alias = "skipped")]
    skip: bool,
    #[serde(default)]
    circuit_breaker: Option<CircuitBreakerConfig>,
}

impl<'de> Deserialize<'de> for GateDef {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let config = GateDefConfig::deserialize(deserializer)?;
        let required = if config.allow_fail || config.skip {
            false
        } else {
            config.required
        };
        let command = if config.skip {
            SKIPPED_GATE_COMMAND.to_string()
        } else {
            config.command
        };
        Ok(Self {
            name: config.name,
            command,
            args: config.args,
            required,
            timeout_secs: config.timeout_secs,
            circuit_breaker: config.circuit_breaker,
        })
    }
}

impl GateDef {
    pub fn new(name: &str, command: &str, args: &[&str]) -> Self {
        Self {
            name: name.to_string(),
            command: command.to_string(),
            args: args.iter().map(|s| s.to_string()).collect(),
            required: true,
            timeout_secs: 0,
            circuit_breaker: None,
        }
    }
}

/// Result of running a single gate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateResult {
    pub name: String,
    pub passed: bool,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u64,
    pub required: bool,
    #[serde(default)]
    pub command_line: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    #[serde(default)]
    pub timed_out: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stdout_summary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stderr_summary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_path: Option<String>,
    #[serde(default)]
    pub timeout_secs: u64,
    #[serde(default)]
    pub circuit_breaker_open: bool,
}

/// Full verification configuration for a project.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VerificationConfig {
    pub gates: Vec<GateDef>,
}

impl VerificationConfig {
    pub fn rust_default() -> Self {
        Self {
            gates: vec![
                GateDef::new("format", "cargo", &["fmt", "--check"]),
                GateDef::new("check", "cargo", &["check", "--all-targets"]),
                GateDef::new("lint", "cargo", &["clippy", "--", "-D", "warnings"]),
                GateDef::new("tests", "cargo", &["test"]),
            ],
        }
    }

    pub fn node_default() -> Self {
        Self {
            gates: vec![
                GateDef::new("tests", "npm", &["test"]),
                GateDef::new("lint", "npm", &["run", "lint"]),
            ],
        }
    }

    pub fn python_default() -> Self {
        Self {
            gates: vec![
                GateDef::new("tests", "python", &["-m", "pytest"]),
                GateDef::new("lint", "python", &["-m", "flake8", "."]),
            ],
        }
    }

    pub fn go_default() -> Self {
        Self {
            gates: vec![
                GateDef::new("format", "gofmt", &["-l", "."]),
                GateDef::new("vet", "go", &["vet", "./..."]),
                GateDef::new("tests", "go", &["test", "./..."]),
            ],
        }
    }
}

/// A durable record of what happened in a run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoneContract {
    pub run_name: String,
    pub mode: String,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub completed_at: chrono::DateTime<chrono::Utc>,
    pub gates: Vec<GateResult>,
    pub changed_files: Vec<String>,
    pub known_gaps: Vec<String>,
    pub passed: bool,
}

impl DoneContract {
    pub fn new(run_name: &str, mode: &str, started_at: chrono::DateTime<chrono::Utc>) -> Self {
        Self {
            run_name: run_name.to_string(),
            mode: mode.to_string(),
            started_at,
            completed_at: chrono::Utc::now(),
            gates: Vec::new(),
            changed_files: Vec::new(),
            known_gaps: Vec::new(),
            passed: false,
        }
    }

    pub async fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let json = serde_json::to_string_pretty(self)?;
        crate::runtime::atomic::atomic_write(path, json.as_bytes()).await?;
        Ok(())
    }

    #[allow(dead_code)]
    pub async fn load(path: &Path) -> Result<Self> {
        let json = tokio::fs::read_to_string(path).await?;
        let contract: DoneContract = serde_json::from_str(&json)?;
        Ok(contract)
    }
}
