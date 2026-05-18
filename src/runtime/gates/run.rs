use std::path::Path;
use std::time::Duration;
use tokio::process::Command;
use tracing::{debug, info, warn};

use crate::runtime::gates::types::{GateDef, GateResult, VerificationConfig, SKIPPED_GATE_COMMAND};
use crate::wire::protocol::scrub_secret_patterns;

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
        info!(gate = %gate.name, command = %gate.command, "Running gate");
        debug!(gate = %gate.name, args = %scrub_secret_patterns(&gate.args.join(" ")), "Running gate args");
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
            cmd.kill_on_drop(true);
            match tokio::time::timeout(Duration::from_secs(gate.timeout_secs), cmd.output()).await {
                Ok(Ok(output)) => output,
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
                    let timeout_message = format!("Timed out after {}s", gate.timeout_secs);
                    warn!(gate = %gate.name, timeout = gate.timeout_secs, "Gate timed out");
                    results.push(make_gate_timeout(
                        gate,
                        &command_line,
                        start,
                        timeout_message,
                    ));
                    continue;
                }
            }
        } else {
            match tokio::time::timeout(Duration::from_secs(60), cmd.output()).await {
                Ok(Ok(o)) => o,
                Ok(Err(e)) => {
                    warn!(gate = %gate.name, error = %e, "Failed to spawn gate command");
                    results.push(make_gate_error(
                        gate,
                        &command_line,
                        start,
                        &format!("Spawn error: {e}"),
                    ));
                    continue;
                }
                Err(_) => {
                    let timeout_message = "Timed out after 60s (default)".to_string();
                    warn!(gate = %gate.name, timeout = 60, "Gate timed out");
                    results.push(make_gate_timeout(
                        gate,
                        &command_line,
                        start,
                        timeout_message,
                    ));
                    continue;
                }
            }
        };

        let stdout = scrub_secret_patterns(&String::from_utf8_lossy(&output.stdout)).into_owned();
        let stderr = scrub_secret_patterns(&String::from_utf8_lossy(&output.stderr)).into_owned();
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

fn make_gate_timeout(
    gate: &GateDef,
    command_line: &str,
    start: std::time::Instant,
    message: String,
) -> GateResult {
    GateResult {
        name: gate.name.clone(),
        passed: false,
        stdout: String::new(),
        stderr: message.clone(),
        duration_ms: start.elapsed().as_millis() as u64,
        required: gate.required,
        command_line: command_line.to_string(),
        exit_code: None,
        timed_out: true,
        stdout_summary: None,
        stderr_summary: Some(message),
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
