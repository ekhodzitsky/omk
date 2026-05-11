use std::path::Path;
use tokio::process::Command;
use tracing::{info, warn};

use crate::runtime::gates::types::{GateResult, VerificationConfig};

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

/// Load explicit gate config if present, otherwise auto-detect.
pub async fn load_or_detect_gates(dir: &Path) -> VerificationConfig {
    let explicit = dir.join(".omk").join("gates.toml");
    if explicit.exists() {
        match tokio::fs::read_to_string(&explicit).await {
            Ok(content) => match toml::from_str(&content) {
                Ok(config) => {
                    info!(path = %explicit.display(), "Loaded explicit gate config");
                    return config;
                }
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

/// Detect changed files using git diff.
pub async fn detect_changed_files(dir: &Path) -> Vec<String> {
    let output = Command::new("git")
        .args(["diff", "--name-only"])
        .current_dir(dir)
        .output()
        .await;

    match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout)
            .lines()
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty())
            .collect(),
        _ => Vec::new(),
    }
}
