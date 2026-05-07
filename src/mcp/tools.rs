use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;
use tracing::info;

use crate::runtime::{
    bridge::TeamBridge,
    state::{TeamPhase, TeamState},
    tmux,
    worker::WorkerSpec,
};
use crate::skills::discovery::load_bundled_skill;

#[derive(Debug, Clone, Serialize)]
pub struct Tool {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

#[derive(Debug, Deserialize)]
pub struct OmkTeamParams {
    pub count: usize,
    pub role: String,
    pub task: String,
}

#[derive(Debug, Deserialize)]
pub struct OmkStatusParams {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct OmkShutdownParams {
    pub name: String,
    #[serde(default)]
    pub force: bool,
}

pub fn list_tools() -> Vec<Tool> {
    vec![
        Tool {
            name: "omk_team".to_string(),
            description: "Spawn a team of Kimi agents in tmux".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "count": { "type": "integer", "description": "Number of workers (1-16)" },
                    "role": { "type": "string", "description": "Worker role (e.g. coder, reviewer)" },
                    "task": { "type": "string", "description": "Task description" }
                },
                "required": ["count", "role", "task"]
            }),
        },
        Tool {
            name: "omk_status".to_string(),
            description: "Get status of a team".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "Team name" }
                },
                "required": ["name"]
            }),
        },
        Tool {
            name: "omk_shutdown".to_string(),
            description: "Shut down a team".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "Team name" },
                    "force": { "type": "boolean", "description": "Force kill without graceful shutdown" }
                },
                "required": ["name"]
            }),
        },
    ]
}

pub async fn handle_tool_call(name: &str, params: Value) -> Result<Value> {
    match name {
        "omk_team" => omk_team(params).await,
        "omk_status" => omk_status(params).await,
        "omk_shutdown" => omk_shutdown(params).await,
        _ => anyhow::bail!("Unknown tool: {}", name),
    }
}

async fn omk_team(params: Value) -> Result<Value> {
    let p: OmkTeamParams = serde_json::from_value(params)?;
    if p.count == 0 || p.count > 16 {
        anyhow::bail!("Worker count must be between 1 and 16");
    }
    if p.task.is_empty() {
        anyhow::bail!("Task description is required");
    }

    let team_name = format!(
        "{}-{}",
        p.role,
        uuid::Uuid::new_v4().to_string().split('-').next().unwrap()
    );

    info!(
        team = %team_name,
        workers = p.count,
        role = %p.role,
        task = %p.task,
        "Starting team via MCP"
    );

    tmux::ensure_tmux()?;

    let state_dir = dirs::home_dir()
        .context("No home directory")?
        .join(".omk")
        .join("state")
        .join("team")
        .join(&team_name);
    tokio::fs::create_dir_all(&state_dir).await?;

    let state = TeamState::new(&team_name, &p.task, &state_dir, p.count, &p.role);
    state.save().await?;

    let session_name = format!("omk-team-{team_name}");
    let window_name = "lead";

    if !tmux::session_exists(&session_name)? {
        tmux::create_session(&session_name, window_name, std::path::Path::new("."))?;
    }

    let skill_md = load_bundled_skill("team").unwrap_or_default();
    let lead_prompt = build_lead_prompt(&p.task, p.count, &p.role, &state_dir, &skill_md);
    tmux::send_keys(
        &session_name,
        window_name,
        &format!("kimi -p {}", shell_escape(&lead_prompt)),
    )?;

    for i in 0..p.count {
        let worker_name = format!("worker-{i}");
        let worker_dir = state_dir.join("workers").join(&worker_name);
        tokio::fs::create_dir_all(&worker_dir).await?;

        let worker_spec = WorkerSpec {
            name: worker_name.clone(),
            role: p.role.clone(),
            inbox: worker_dir.join("inbox.jsonl"),
            outbox: worker_dir.join("outbox.jsonl"),
            heartbeat: worker_dir.join("heartbeat.json"),
        };
        worker_spec.save().await?;

        let bridge = TeamBridge::new(&worker_spec, &session_name);
        bridge.spawn_worker(i + 1).await?;
    }

    tmux::select_layout(&session_name, window_name, "tiled")?;

    Ok(serde_json::json!({
        "success": true,
        "team": team_name,
        "session": session_name,
        "workers": p.count,
        "role": p.role,
        "state_dir": state_dir.display().to_string(),
    }))
}

async fn omk_status(params: Value) -> Result<Value> {
    let p: OmkStatusParams = serde_json::from_value(params)?;
    let state_dir = dirs::home_dir()
        .context("No home directory")?
        .join(".omk")
        .join("state")
        .join("team")
        .join(&p.name);

    if !state_dir.exists() {
        anyhow::bail!("Team '{}' not found", p.name);
    }

    let state = TeamState::load(&state_dir).await?;
    let session_name = format!("omk-team-{}", p.name);
    let tmux_alive = tmux::session_exists(&session_name).unwrap_or(false);

    let mut workers = vec![];
    let workers_dir = state_dir.join("workers");
    if workers_dir.exists() {
        let mut entries = tokio::fs::read_dir(&workers_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let worker_dir = entry.path();
            let spec_path = worker_dir.join("worker-spec.json");
            if !spec_path.exists() {
                continue;
            }
            let spec: WorkerSpec = {
                let json = tokio::fs::read_to_string(&spec_path).await?;
                serde_json::from_str(&json)?
            };
            let hb_status = if spec.heartbeat.exists() {
                match tokio::fs::read_to_string(&spec.heartbeat).await {
                    Ok(json) => match serde_json::from_str::<Value>(&json) {
                        Ok(v) => v
                            .get("status")
                            .and_then(|s| s.as_str())
                            .unwrap_or("unknown")
                            .to_string(),
                        Err(_) => "invalid".to_string(),
                    },
                    Err(_) => "unreadable".to_string(),
                }
            } else {
                "missing".to_string()
            };
            let inbox_count = count_jsonl_lines(&spec.inbox).await;
            let outbox_count = count_jsonl_lines(&spec.outbox).await;
            workers.push(serde_json::json!({
                "name": spec.name,
                "role": spec.role,
                "heartbeat": hb_status,
                "inbox": inbox_count,
                "outbox": outbox_count,
            }));
        }
    }

    Ok(serde_json::json!({
        "name": state.name,
        "task": state.task,
        "phase": format!("{:?}", state.phase),
        "created_at": state.created_at,
        "tmux_alive": tmux_alive,
        "workers": workers,
        "tasks": state.tasks.iter().map(|t| serde_json::json!({
            "id": t.id,
            "description": t.description,
            "status": format!("{:?}", t.status),
        })).collect::<Vec<_>>(),
    }))
}

async fn omk_shutdown(params: Value) -> Result<Value> {
    let p: OmkShutdownParams = serde_json::from_value(params)?;
    let state_dir = dirs::home_dir()
        .context("No home directory")?
        .join(".omk")
        .join("state")
        .join("team")
        .join(&p.name);

    if !state_dir.exists() {
        anyhow::bail!("Team '{}' not found", p.name);
    }

    let session_name = format!("omk-team-{}", p.name);

    if tmux::session_exists(&session_name)? {
        if !p.force {
            let _ = tmux::send_keys(&session_name, "lead", "C-c");
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }
        tmux::kill_session(&session_name)?;
    }

    let mut state = TeamState::load(&state_dir).await?;
    state.phase = TeamPhase::Shutdown;
    state.save().await?;

    Ok(serde_json::json!({
        "success": true,
        "team": p.name,
        "session": session_name,
        "state_dir": state_dir.display().to_string(),
    }))
}

fn build_lead_prompt(
    task: &str,
    count: usize,
    role: &str,
    state_dir: &std::path::Path,
    skill_md: &str,
) -> String {
    format!(
        r#"You are the Lead Orchestrator of a team of {count} {role} agent(s).

Your task: {task}

## Orchestration Skill
{skill_md}

## State Directory
{state_dir}

## Worker Inbox/Outbox Paths
{inbox_dir}

## Available Tools
- ReadFile / WriteFile for inbox/outbox
- Shell for tmux commands if needed
- TaskList / TaskOutput for background work
"#,
        count = count,
        role = role,
        task = task,
        skill_md = skill_md,
        state_dir = state_dir.display(),
        inbox_dir = state_dir.join("workers").display(),
    )
}

fn shell_escape(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\"'\"'"))
}

async fn count_jsonl_lines(path: &PathBuf) -> usize {
    if !path.exists() {
        return 0;
    }
    match tokio::fs::read_to_string(path).await {
        Ok(content) => content.lines().filter(|l| !l.trim().is_empty()).count(),
        Err(_) => 0,
    }
}
