use crate::error::OmkError;
use serde_json::Value;
use tokio::process::Command;

pub fn list_tools() -> Vec<Value> {
    vec![
        serde_json::json!({
            "name": "omk_team_run",
            "description": "Run a scheduler-backed Kimi team and return execution details.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "spec": { "type": "string", "description": "Worker spec, e.g. '3:coder'" },
                    "task": { "type": "string", "description": "Task description" },
                    "name": { "type": "string", "description": "Optional team name" }
                },
                "required": ["spec", "task"]
            }
        }),
        serde_json::json!({
            "name": "omk_team_status",
            "description": "Check team status including worker heartbeats and task counts",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "Team name" }
                },
                "required": ["name"]
            }
        }),
        serde_json::json!({
            "name": "omk_team_shutdown",
            "description": "Shutdown a team gracefully or forcefully",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "Team name" },
                    "force": { "type": "boolean", "description": "Force shutdown handling", "default": false }
                },
                "required": ["name"]
            }
        }),
        serde_json::json!({
            "name": "omk_doctor",
            "description": "Run environment diagnostics and return a health report",
            "inputSchema": {
                "type": "object",
                "properties": {}
            }
        }),
    ]
}

pub async fn handle_tool_call(name: &str, arguments: Value) -> Result<Value, OmkError> {
    let omk_bin = std::env::current_exe().unwrap_or_else(|_| "omk".into());

    match name {
        "omk_team_run" => {
            let spec = arguments["spec"].as_str().unwrap_or("1:coder");
            let task = arguments["task"].as_str().unwrap_or("");
            let name_arg = arguments["name"].as_str();

            let mut cmd = Command::new(&omk_bin);
            cmd.arg("team").arg("run").arg(spec).arg(task);
            if let Some(n) = name_arg {
                cmd.args(["--name", n]);
            }

            let output = tokio::time::timeout(
                std::time::Duration::from_secs(60),
                cmd.output(),
            )
            .await
            .map_err(|_| OmkError::Timeout { secs: 60 })?
            .map_err(|_e| OmkError::ShellFailed {
                command: format!("omk team run {}", spec),
            })?;
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);

            Ok(serde_json::json!({
                "status": if output.status.success() { "completed" } else { "error" },
                "stdout": stdout.to_string(),
                "stderr": stderr.to_string(),
                "spec": spec,
                "task": task,
            }))
        }

        "omk_team_status" => {
            let team_name = arguments["name"].as_str().unwrap_or("");

            let output = tokio::time::timeout(
                std::time::Duration::from_secs(60),
                Command::new(&omk_bin)
                    .args(["team", "status", team_name])
                    .output(),
            )
            .await
            .map_err(|_| OmkError::Timeout { secs: 60 })?
            .map_err(|_e| OmkError::ShellFailed {
                command: format!("omk team status {}", team_name),
            })?;

            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);

            Ok(serde_json::json!({
                "status": if output.status.success() { "ok" } else { "error" },
                "team": team_name,
                "stdout": stdout.to_string(),
                "stderr": stderr.to_string(),
            }))
        }

        "omk_team_shutdown" => {
            let team_name = arguments["name"].as_str().unwrap_or("");
            let force = arguments["force"].as_bool().unwrap_or(false);

            let mut cmd = Command::new(&omk_bin);
            cmd.arg("team").arg("shutdown").arg(team_name);
            if force {
                cmd.arg("--force");
            }

            let output = tokio::time::timeout(
                std::time::Duration::from_secs(60),
                cmd.output(),
            )
            .await
            .map_err(|_| OmkError::Timeout { secs: 60 })?
            .map_err(|_e| OmkError::ShellFailed {
                command: format!("omk team shutdown {}", team_name),
            })?;
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);

            Ok(serde_json::json!({
                "status": if output.status.success() { "shutdown" } else { "error" },
                "team": team_name,
                "force": force,
                "stdout": stdout.to_string(),
                "stderr": stderr.to_string(),
            }))
        }

        "omk_doctor" => {
            let output = tokio::time::timeout(
                std::time::Duration::from_secs(60),
                Command::new(&omk_bin).arg("doctor").output(),
            )
            .await
            .map_err(|_| OmkError::Timeout { secs: 60 })?
            .map_err(|_e| OmkError::ShellFailed {
                command: "omk doctor".to_string(),
            })?;

            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let combined = format!("{}{}", stdout, stderr);
            let healthy = combined.contains("All checks passed");

            Ok(serde_json::json!({
                "status": if healthy { "healthy" } else { "issues_found" },
                "healthy": healthy,
                "stdout": stdout.to_string(),
                "stderr": stderr.to_string(),
            }))
        }

        _ => Err(OmkError::InvalidInput {
            reason: format!("Unknown MCP tool: {}", name),
        }),
    }
}
