use std::collections::HashSet;
use std::path::Path;
use std::time::Duration;
use tokio::process::Command;
use tracing::{info, warn};

use crate::runtime::gates::types::{GateResult, VerificationConfig};

/// Maximum allowed timeout for a gate in seconds (24 hours).
const MAX_TIMEOUT_SECS: u64 = 86400;

/// Detect project type and return default gates.
pub fn detect_gates(dir: &Path) -> VerificationConfig {
    if dir.join("Cargo.toml").exists() {
        VerificationConfig::rust_default()
    } else if dir.join("package.json").exists() {
        VerificationConfig::node_default()
    } else if dir.join("go.mod").exists() {
        VerificationConfig::go_default()
    } else if dir.join("pyproject.toml").exists()
        || dir.join("setup.py").exists()
        || dir.join("requirements.txt").exists()
    {
        VerificationConfig::python_default()
    } else {
        VerificationConfig::default()
    }
}

/// Validate a gate configuration loaded from TOML.
///
/// Returns `Ok(())` if the config is valid, or `Err` with a description
/// of the first problem encountered.
fn validate_config(config: &VerificationConfig) -> Result<(), String> {
    let mut seen_names = HashSet::new();

    for gate in &config.gates {
        if gate.name.trim().is_empty() {
            return Err("gate name must not be empty".to_string());
        }
        if !seen_names.insert(&gate.name) {
            return Err(format!("duplicate gate name: {}", gate.name));
        }
        if gate.command.trim().is_empty() {
            return Err(format!("gate '{}' command must not be empty", gate.name));
        }
        if gate.timeout_secs > MAX_TIMEOUT_SECS {
            return Err(format!(
                "gate '{}' timeout_secs {} exceeds maximum {}",
                gate.name, gate.timeout_secs, MAX_TIMEOUT_SECS
            ));
        }
    }

    Ok(())
}

/// Load explicit gate config if present, otherwise auto-detect.
pub async fn load_or_detect_gates(dir: &Path) -> VerificationConfig {
    let explicit = dir.join(".omk").join("gates.toml");
    if explicit.exists() {
        match tokio::fs::read_to_string(&explicit).await {
            Ok(content) => match toml::from_str(&content) {
                Ok(config) => match validate_config(&config) {
                    Ok(()) => {
                        info!(path = %explicit.display(), "Loaded explicit gate config");
                        return config;
                    }
                    Err(e) => {
                        warn!(path = %explicit.display(), error = %e, "Invalid gates.toml schema, falling back to auto-detect");
                    }
                },
                Err(e) => {
                    warn!(path = %explicit.display(), error = %e, "Failed to parse gates.toml, falling back to auto-detect");
                }
            },
            Err(e) => {
                warn!(path = %explicit.display(), error = %e, "Failed to read gates.toml, falling back to auto-detect");
            }
        }
    }
    detect_gates(dir)
}

/// Summary of gate results.
pub fn gates_passed(results: &[GateResult]) -> bool {
    results.iter().all(|r| !r.required || r.passed)
}

pub fn format_gate_summary(results: &[GateResult]) -> String {
    let mut summary = String::from("Verification Gates:\n");
    for r in results {
        let icon = if r.passed { "✓" } else { "✗" };
        let req = if r.required { "required" } else { "optional" };
        summary.push_str(&format!(
            "  {} {} ({}, {}ms)\n",
            icon, r.name, req, r.duration_ms
        ));
        if !r.passed && !r.stderr.is_empty() {
            for line in r.stderr.lines().take(3) {
                summary.push_str(&format!("    > {}\n", line));
            }
        }
    }
    summary
}

/// Detect changed files using git status, including untracked files.
pub async fn detect_changed_files(dir: &Path) -> Vec<String> {
    let mut cmd = Command::new("git");
    cmd.args(["status", "--porcelain"]).current_dir(dir);

    let output = tokio::time::timeout(Duration::from_secs(10), cmd.output()).await;

    let mut files = match output {
        Ok(Ok(o)) if o.status.success() => String::from_utf8_lossy(&o.stdout)
            .lines()
            .filter_map(parse_porcelain_changed_file)
            .collect::<Vec<_>>(),
        _ => Vec::new(),
    };
    files.sort();
    files.dedup();
    files
}

/// Synchronous variant for non-async contexts (e.g. review passes).
pub fn detect_changed_files_sync(dir: &Path) -> Vec<String> {
    let output = std::process::Command::new("git")
        .args(["status", "--porcelain", "--untracked-files=all"])
        .current_dir(dir)
        .output();

    let mut files = match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout)
            .lines()
            .filter_map(parse_porcelain_changed_file)
            .collect::<Vec<_>>(),
        _ => Vec::new(),
    };
    files.sort();
    files.dedup();
    files
}

pub fn parse_porcelain_changed_file(line: &str) -> Option<String> {
    if line.len() < 4 {
        return None;
    }

    let path = line.get(3..)?.trim();
    if path.is_empty() {
        return None;
    }

    let path = path.split(" -> ").last().unwrap_or(path).trim_matches('"');
    if path.is_empty() {
        None
    } else {
        Some(path.to_string())
    }
}
