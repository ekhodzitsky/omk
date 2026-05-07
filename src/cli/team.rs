use anyhow::{Context, Result};
use clap::Parser;
use std::path::PathBuf;
use tracing::info;

use crate::runtime::{bridge::TeamBridge, state::TeamState, tmux, worker::WorkerSpec};

/// Spawn a team of Kimi agents in tmux
#[derive(Parser, Debug, Clone)]
pub struct Args {
    /// Team specification, e.g. "3:coder" or "2:executor"
    #[arg(value_name = "N:ROLE")]
    pub spec: String,

    /// Task description
    #[arg(trailing_var_arg = true, value_name = "TASK")]
    pub task: Vec<String>,

    /// Team name (auto-generated if omitted)
    #[arg(short, long)]
    pub name: Option<String>,

    /// Working directory
    #[arg(short, long, default_value = ".")]
    pub dir: PathBuf,

    /// Disable Ralph loop (run once)
    #[arg(long)]
    pub no_ralph: bool,

    /// YOLO mode (auto-approve)
    #[arg(long)]
    pub yolo: bool,
}

pub async fn run(args: Args) -> Result<()> {
    let task = args.task.join(" ");
    if task.is_empty() {
        anyhow::bail!("Task description is required");
    }

    let (count, role) = parse_spec(&args.spec)?;
    let team_name = args
        .name
        .unwrap_or_else(|| format!("{}-{}", role, uuid::Uuid::new_v4().to_string().split('-').next().unwrap()));

    info!(team = %team_name, workers = count, role = role, task = %task, "Starting team");

    // Ensure tmux is available
    tmux::ensure_tmux()?;

    // Create state directory
    let state_dir = dirs::home_dir()
        .context("No home directory")?
        .join(".omk")
        .join("state")
        .join("team")
        .join(&team_name);
    tokio::fs::create_dir_all(&state_dir).await?;

    let state = TeamState::new(&team_name, &task, &state_dir, count, &role);
    state.save().await?;

    // Create or attach tmux session
    let session_name = format!("omk-team-{team_name}");
    let window_name = "lead";

    if !tmux::session_exists(&session_name)? {
        tmux::create_session(&session_name, window_name, &args.dir)?;
    }

    // Spawn lead agent in first pane
    let lead_prompt = build_lead_prompt(&task, count, &role, &state_dir, args.yolo);
    tmux::send_keys(&session_name, window_name, &format!("kimi -p {}", shell_escape(&lead_prompt)))?;

    // Spawn worker panes
    for i in 0..count {
        let worker_name = format!("worker-{i}");
        let worker_dir = state_dir.join("workers").join(&worker_name);
        tokio::fs::create_dir_all(&worker_dir).await?;

        tmux::split_window(&session_name, window_name, &args.dir)?;
        tmux::rename_pane(&session_name, window_name, i + 1, &worker_name)?;

        let worker_spec = WorkerSpec {
            name: worker_name.clone(),
            role: role.clone(),
            inbox: worker_dir.join("inbox.jsonl"),
            outbox: worker_dir.join("outbox.jsonl"),
            heartbeat: worker_dir.join("heartbeat.json"),
        };
        worker_spec.save().await?;

        let bridge = TeamBridge::new(&worker_spec, &session_name);
        bridge.spawn_worker(i + 1).await?;
    }

    // Arrange layout
    tmux::select_layout(&session_name, window_name, "tiled")?;

    println!("✓ Team '{}' started with {} {} worker(s)", team_name, count, role);
    println!("  Session: {}", session_name);
    println!("  State:   {}", state_dir.display());
    println!();
    println!("Commands:");
    println!("  omk team status {team_name}");
    println!("  omk team shutdown {team_name}");
    println!();
    println!("Attach with: tmux attach -t {}", session_name);

    Ok(())
}

fn parse_spec(spec: &str) -> Result<(usize, String)> {
    let parts: Vec<&str> = spec.splitn(2, ':').collect();
    if parts.len() != 2 {
        anyhow::bail!("Invalid spec '{}'. Expected format: N:role (e.g. 3:coder)", spec);
    }
    let count: usize = parts[0]
        .parse()
        .with_context(|| format!("Invalid worker count '{}'", parts[0]))?;
    if count == 0 || count > 16 {
        anyhow::bail!("Worker count must be between 1 and 16");
    }
    Ok((count, parts[1].to_string()))
}

fn build_lead_prompt(task: &str, count: usize, role: &str, state_dir: &std::path::Path, yolo: bool) -> String {
    let mut prompt = format!(
        r#"You are the Lead Orchestrator of a team of {count} {role} agent(s).

Your task: {task}

## Team Coordination Rules
1. Decompose the task into subtasks suitable for parallel execution.
2. Write each subtask as a JSON object to the worker inbox files at {inbox_dir}
3. Wait for outbox results and synthesize the final answer.
4. If a worker fails, reassign or fix the subtask.

## Worker Inbox Format (JSONL)
Each line: {{"id":"uuid","task":"description","acceptance_criteria":["..."]}}

## State Directory
{state_dir}

## Available Tools
- ReadFile / WriteFile for inbox/outbox
- Shell for tmux commands if needed
- TaskList / TaskOutput for background work
"#,
        count = count,
        role = role,
        task = task,
        inbox_dir = state_dir.join("workers").display(),
        state_dir = state_dir.display(),
    );

    if yolo {
        prompt.push_str("\n\nYOLO mode is enabled. Auto-approve safe operations.\n");
    }

    prompt
}

fn shell_escape(s: &str) -> String {
    // Simple shell escape: wrap in single quotes, escape existing single quotes
    format!("'{}'", s.replace('\'', "'\"'\"'"))
}
