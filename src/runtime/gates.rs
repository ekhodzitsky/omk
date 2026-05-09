use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::process::Command;
use tracing::{info, warn};

/// A single verification gate definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateDef {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    /// If true, failure blocks completion.
    #[serde(default = "default_required")]
    pub required: bool,
    /// Timeout in seconds. 0 means no timeout.
    #[serde(default)]
    pub timeout_secs: u64,
}

fn default_required() -> bool {
    true
}

impl GateDef {
    pub fn new(name: &str, command: &str, args: &[&str]) -> Self {
        Self {
            name: name.to_string(),
            command: command.to_string(),
            args: args.iter().map(|s| s.to_string()).collect(),
            required: true,
            timeout_secs: 120,
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

/// Run all configured gates and return results.
pub async fn run_gates(config: &VerificationConfig, dir: &Path) -> Vec<GateResult> {
    run_gates_with_evidence(config, dir, None).await
}

/// Run all configured gates and optionally persist full stdout/stderr artifacts.
pub async fn run_gates_with_evidence(
    config: &VerificationConfig,
    dir: &Path,
    output_dir: Option<&Path>,
) -> Vec<GateResult> {
    let mut results = Vec::with_capacity(config.gates.len());

    for (index, gate) in config.gates.iter().enumerate() {
        let start = std::time::Instant::now();
        info!(gate = %gate.name, command = %gate.command, args = ?gate.args, "Running gate");
        let command_line = render_command_line(&gate.command, &gate.args);

        let mut cmd = Command::new(&gate.command);
        cmd.args(&gate.args).current_dir(dir);

        let output = if gate.timeout_secs > 0 {
            let mut child = match cmd
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
            {
                Ok(c) => c,
                Err(e) => {
                    warn!(gate = %gate.name, error = %e, "Failed to spawn gate command");
                    results.push(GateResult {
                        name: gate.name.clone(),
                        passed: false,
                        stdout: String::new(),
                        stderr: format!("Spawn error: {e}"),
                        duration_ms: start.elapsed().as_millis() as u64,
                        required: gate.required,
                        command_line: command_line.clone(),
                        exit_code: None,
                        timed_out: false,
                        stdout_summary: None,
                        stderr_summary: Some(format!("Spawn error: {e}")),
                        output_path: None,
                        timeout_secs: gate.timeout_secs,
                    });
                    continue;
                }
            };
            match tokio::time::timeout(
                std::time::Duration::from_secs(gate.timeout_secs),
                child.wait(),
            )
            .await
            {
                Ok(Ok(status)) => {
                    let mut stdout = Vec::new();
                    let mut stderr = Vec::new();
                    if let Some(mut out) = child.stdout.take() {
                        let _ = tokio::io::AsyncReadExt::read_to_end(&mut out, &mut stdout).await;
                    }
                    if let Some(mut err) = child.stderr.take() {
                        let _ = tokio::io::AsyncReadExt::read_to_end(&mut err, &mut stderr).await;
                    }
                    std::process::Output {
                        status,
                        stdout,
                        stderr,
                    }
                }
                Ok(Err(e)) => {
                    warn!(gate = %gate.name, error = %e, "Failed to run gate command");
                    results.push(GateResult {
                        name: gate.name.clone(),
                        passed: false,
                        stdout: String::new(),
                        stderr: format!("Run error: {e}"),
                        duration_ms: start.elapsed().as_millis() as u64,
                        required: gate.required,
                        command_line: command_line.clone(),
                        exit_code: None,
                        timed_out: false,
                        stdout_summary: None,
                        stderr_summary: Some(format!("Run error: {e}")),
                        output_path: None,
                        timeout_secs: gate.timeout_secs,
                    });
                    continue;
                }
                Err(_) => {
                    let _ = child.kill().await;
                    let _ = child.wait().await;
                    let timeout_message = format!("Timed out after {}s", gate.timeout_secs);
                    warn!(gate = %gate.name, timeout = gate.timeout_secs, "Gate timed out");
                    results.push(GateResult {
                        name: gate.name.clone(),
                        passed: false,
                        stdout: String::new(),
                        stderr: timeout_message.clone(),
                        duration_ms: start.elapsed().as_millis() as u64,
                        required: gate.required,
                        command_line: command_line.clone(),
                        exit_code: None,
                        timed_out: true,
                        stdout_summary: None,
                        stderr_summary: Some(timeout_message),
                        output_path: None,
                        timeout_secs: gate.timeout_secs,
                    });
                    continue;
                }
            }
        } else {
            match cmd.output().await {
                Ok(o) => o,
                Err(e) => {
                    warn!(gate = %gate.name, error = %e, "Failed to spawn gate command");
                    results.push(GateResult {
                        name: gate.name.clone(),
                        passed: false,
                        stdout: String::new(),
                        stderr: format!("Spawn error: {e}"),
                        duration_ms: start.elapsed().as_millis() as u64,
                        required: gate.required,
                        command_line: command_line.clone(),
                        exit_code: None,
                        timed_out: false,
                        stdout_summary: None,
                        stderr_summary: Some(format!("Spawn error: {e}")),
                        output_path: None,
                        timeout_secs: gate.timeout_secs,
                    });
                    continue;
                }
            }
        };

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let passed = output.status.success();
        let exit_code = output.status.code();
        let duration_ms = start.elapsed().as_millis() as u64;
        let stdout_summary = summarize_output(&stdout);
        let stderr_summary = summarize_output(&stderr);
        let output_path = if let Some(dir) = output_dir {
            write_full_output_artifact(dir, &gate.name, index, &stdout, &stderr).await
        } else {
            None
        };

        info!(
            gate = %gate.name,
            passed,
            duration_ms,
            "Gate complete"
        );

        results.push(GateResult {
            name: gate.name.clone(),
            passed,
            stdout,
            stderr,
            duration_ms,
            required: gate.required,
            command_line,
            exit_code,
            timed_out: false,
            stdout_summary,
            stderr_summary,
            output_path,
            timeout_secs: gate.timeout_secs,
        });
    }

    results
}

fn render_command_line(command: &str, args: &[String]) -> String {
    if args.is_empty() {
        command.to_string()
    } else {
        format!("{command} {}", args.join(" "))
    }
}

fn summarize_output(text: &str) -> Option<String> {
    let mut lines: Vec<String> = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .take(3)
        .map(|line| {
            let mut out = line.to_string();
            if out.chars().count() > 240 {
                out = format!("{}...", out.chars().take(240).collect::<String>());
            }
            out
        })
        .collect();
    if lines.is_empty() {
        return None;
    }
    if text.lines().count() > 3 {
        lines.push("...".to_string());
    }
    Some(lines.join("\n"))
}

async fn write_full_output_artifact(
    output_dir: &Path,
    gate_name: &str,
    gate_index: usize,
    stdout: &str,
    stderr: &str,
) -> Option<String> {
    if tokio::fs::create_dir_all(output_dir).await.is_err() {
        return None;
    }
    let safe_name = gate_name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>();
    let file_name = format!("gate-{:02}-{}.log", gate_index + 1, safe_name);
    let path = output_dir.join(file_name);
    let body = format!(
        "[stdout]\n{}\n\n[stderr]\n{}\n",
        stdout.trim_end(),
        stderr.trim_end()
    );
    if tokio::fs::write(&path, body).await.is_ok() {
        Some(path.to_string_lossy().to_string())
    } else {
        None
    }
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

// ------------------------------------------------------------------
// Done Contract
// ------------------------------------------------------------------

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
