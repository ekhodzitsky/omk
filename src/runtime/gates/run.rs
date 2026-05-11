use std::path::Path;
use tokio::process::Command;
use tracing::{info, warn};

use crate::runtime::gates::types::{GateDef, GateResult, VerificationConfig};

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
    const SKIPPED_GATE_COMMAND: &str = "__omk_internal_skipped_gate__";
    let mut results = Vec::with_capacity(config.gates.len());

    for (index, gate) in config.gates.iter().enumerate() {
        let start = std::time::Instant::now();
        info!(gate = %gate.name, command = %gate.command, args = ?gate.args, "Running gate");
        if gate.command == SKIPPED_GATE_COMMAND {
            let skipped_message = "Skipped by gate config".to_string();
            results.push(GateResult {
                name: gate.name.clone(),
                passed: true,
                stdout: String::new(),
                stderr: skipped_message.clone(),
                duration_ms: start.elapsed().as_millis() as u64,
                required: gate.required,
                command_line: "<skipped by config>".to_string(),
                exit_code: None,
                timed_out: false,
                stdout_summary: None,
                stderr_summary: Some(skipped_message),
                output_path: None,
                timeout_secs: gate.timeout_secs,
            });
            continue;
        }
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
                    results.push(make_gate_error(
                        gate,
                        &command_line,
                        start,
                        &format!("Spawn error: {e}"),
                    ));
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
                    results.push(make_gate_error(
                        gate,
                        &command_line,
                        start,
                        &format!("Run error: {e}"),
                    ));
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
                    results.push(make_gate_error(
                        gate,
                        &command_line,
                        start,
                        &format!("Spawn error: {e}"),
                    ));
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

fn make_gate_error(
    gate: &GateDef,
    command_line: &str,
    start: std::time::Instant,
    message: &str,
) -> GateResult {
    GateResult {
        name: gate.name.clone(),
        passed: false,
        stdout: String::new(),
        stderr: message.to_string(),
        duration_ms: start.elapsed().as_millis() as u64,
        required: gate.required,
        command_line: command_line.to_string(),
        exit_code: None,
        timed_out: false,
        stdout_summary: None,
        stderr_summary: Some(message.to_string()),
        output_path: None,
        timeout_secs: gate.timeout_secs,
    }
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
