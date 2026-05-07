use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tracing::info;

use crate::runtime::{bridge::TeamBridge, state::TeamState, tmux, worker::WorkerSpec};
use crate::skills::discovery::load_bundled_skill;

#[derive(Parser, Debug, Clone)]
pub struct Args {
    #[command(subcommand)]
    pub command: TeamCommands,
}

#[derive(Subcommand, Debug, Clone)]
pub enum TeamCommands {
    /// Spawn a team of Kimi agents in tmux
    Spawn(SpawnArgs),
    /// Check team status
    Status(StatusArgs),
    /// Shutdown a team
    Shutdown(ShutdownArgs),
}

#[derive(Parser, Debug, Clone)]
pub struct SpawnArgs {
    #[arg(value_name = "N:ROLE")]
    pub spec: String,

    #[arg(trailing_var_arg = true, value_name = "TASK")]
    pub task: Vec<String>,

    #[arg(short, long)]
    pub name: Option<String>,

    #[arg(short, long, default_value = ".")]
    pub dir: PathBuf,

    #[arg(long)]
    pub no_ralph: bool,

    #[arg(long)]
    pub yolo: bool,

    /// Skill to inject into lead prompt
    #[arg(short, long, default_value = "team")]
    pub skill: String,
}

#[derive(Parser, Debug, Clone)]
pub struct StatusArgs {
    #[arg(value_name = "NAME")]
    pub name: String,
}

#[derive(Parser, Debug, Clone)]
pub struct ShutdownArgs {
    #[arg(value_name = "NAME")]
    pub name: String,

    #[arg(long)]
    pub force: bool,
}

pub async fn run(args: Args) -> Result<()> {
    match args.command {
        TeamCommands::Spawn(args) => spawn(args).await,
        TeamCommands::Status(args) => status(args).await,
        TeamCommands::Shutdown(args) => shutdown(args).await,
    }
}

async fn spawn(args: SpawnArgs) -> Result<()> {
    let task = args.task.join(" ");
    if task.is_empty() {
        anyhow::bail!("Task description is required");
    }

    let (count, role) = parse_spec(&args.spec)?;
    let team_name = args
        .name
        .unwrap_or_else(|| format!("{}-{}", role, uuid::Uuid::new_v4().to_string().split('-').next().unwrap()));

    info!(team = %team_name, workers = count, role = role, task = %task, "Starting team");

    tmux::ensure_tmux()?;

    let state_dir = crate::runtime::config::omk_state_dir()
        .join("team")
        .join(&team_name);
    tokio::fs::create_dir_all(&state_dir).await?;

    let state = TeamState::new(&team_name, &task, &state_dir, count, &role);
    state.save().await?;

    let session_name = format!("omk-team-{team_name}");
    let window_name = "lead";

    if !tmux::session_exists(&session_name)? {
        tmux::create_session(&session_name, window_name, &args.dir)?;
    }

    let skill_md = load_bundled_skill(&args.skill).unwrap_or_default();
    let lead_prompt = build_lead_prompt(&task, count, &role, &state_dir, args.yolo, &skill_md);
    crate::runtime::shell::validate_safe(&lead_prompt)
        .map_err(|e| anyhow::anyhow!("Invalid prompt: {}", e))?;
    tmux::send_keys(&session_name, window_name, &format!("kimi -p {}", crate::runtime::shell::shell_escape(&lead_prompt)))?;

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

async fn status(args: StatusArgs) -> Result<()> {
    let state_dir = crate::runtime::config::omk_state_dir()
        .join("team")
        .join(&args.name);

    if !state_dir.exists() {
        anyhow::bail!("Team '{}' not found. Expected state at: {}", args.name, state_dir.display());
    }

    let state = TeamState::load(&state_dir).await?;
    let session_name = format!("omk-team-{}", args.name);
    let tmux_alive = tmux::session_exists(&session_name).unwrap_or(false);

    println!("Team:        {}", state.name);
    println!("Task:        {}", state.task);
    println!("Phase:       {:?}", state.phase);
    println!("Created:     {}", state.created_at);
    println!("Tmux:        {}", if tmux_alive { "running" } else { "not found" });
    println!();
    println!("Workers:");

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
                    Ok(json) => match serde_json::from_str::<serde_json::Value>(&json) {
                        Ok(v) => v.get("status").and_then(|s| s.as_str()).unwrap_or("unknown").to_string(),
                        Err(_) => "invalid".to_string(),
                    },
                    Err(_) => "unreadable".to_string(),
                }
            } else {
                "missing".to_string()
            };

            let inbox_count = count_jsonl_lines(&spec.inbox).await;
            let outbox_count = count_jsonl_lines(&spec.outbox).await;

            println!(
                "  {:12} role={:10} hb={:8} inbox={:2} outbox={:2}",
                spec.name, spec.role, hb_status, inbox_count, outbox_count
            );
        }
    }

    println!();
    println!("Tasks:       {} total", state.tasks.len());
    for task in &state.tasks {
        println!("  [{:?}] {}", task.status, task.description);
    }

    Ok(())
}

async fn shutdown(args: ShutdownArgs) -> Result<()> {
    let state_dir = crate::runtime::config::omk_state_dir()
        .join("team")
        .join(&args.name);

    if !state_dir.exists() {
        anyhow::bail!("Team '{}' not found. Expected state at: {}", args.name, state_dir.display());
    }

    let session_name = format!("omk-team-{}", args.name);

    if tmux::session_exists(&session_name)? {
        if !args.force {
            println!("Sending interrupt to team '{}'...", args.name);
            // Send Ctrl-C to all panes
            let _ = tmux::send_keys(&session_name, "lead", "C-c");
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }

        tmux::kill_session(&session_name)?;
        println!("✓ Tmux session '{}' killed", session_name);
    } else {
        println!("⚠ Tmux session '{}' not found (already dead?)", session_name);
    }

    // Update state
    let mut state = TeamState::load(&state_dir).await?;
    state.phase = crate::runtime::state::TeamPhase::Shutdown;
    state.save().await?;

    println!("✓ Team '{}' shut down", args.name);
    println!("  State:   {}", state_dir.display());

    Ok(())
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

fn build_lead_prompt(task: &str, count: usize, role: &str, state_dir: &std::path::Path, yolo: bool, skill_md: &str) -> String {
    let mut prompt = format!(
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
    );

    if yolo {
        prompt.push_str("\n\nYOLO mode is enabled. Auto-approve safe operations.\n");
    }

    prompt
}

