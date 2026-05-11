use anyhow::Result;
use std::path::PathBuf;

use crate::cli::team::args::{HealthArgs, StatusArgs};

use crate::runtime::config::{EVENTS_FILE, TEAM_DIR, WORKERS_DIR};
use crate::runtime::sanitize::sanitize_name;
use crate::runtime::state::TeamState;
use crate::runtime::worker::WorkerSpec;

pub(crate) async fn list_teams() -> Result<()> {
    let teams_dir = crate::runtime::config::omk_state_dir().join(TEAM_DIR);

    if !teams_dir.exists() {
        println!("No teams found.");
        return Ok(());
    }

    let mut entries = tokio::fs::read_dir(&teams_dir).await?;
    let mut teams = Vec::new();

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path.is_dir() {
            let name = entry.file_name().to_string_lossy().to_string();

            if let Ok(state) = TeamState::load(&path).await {
                teams.push((name, state));
            }
        }
    }

    if teams.is_empty() {
        println!("No teams found.");
        return Ok(());
    }

    println!("Active teams:\n");
    println!("{:<20} {:<20} Task", "Name", "Phase");
    println!("{}", "─".repeat(78));

    for (name, state) in teams {
        println!(
            "{:<20} {:<20} {}",
            name,
            format!("{:?}", state.phase),
            state.task.chars().take(40).collect::<String>()
        );
    }

    println!("\nUse `omk team status <name>` for details.");
    Ok(())
}

pub(crate) async fn status(args: StatusArgs) -> Result<()> {
    let team_name = sanitize_name(&args.name)?;
    let state_dir = crate::runtime::config::omk_state_dir()
        .join(TEAM_DIR)
        .join(&team_name);

    if !state_dir.exists() {
        anyhow::bail!(
            "Team '{}' not found. Expected state at: {}",
            team_name,
            state_dir.display()
        );
    }

    let state = TeamState::load(&state_dir).await?;

    println!("Team:        {}", state.name);
    println!("Task:        {}", state.task);
    println!("Phase:       {:?}", state.phase);
    println!("Created:     {}", state.created_at);
    println!();
    println!("Workers:");

    let workers_dir = state_dir.join(WORKERS_DIR);
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

pub(crate) async fn health(args: HealthArgs) -> Result<()> {
    let team_name = sanitize_name(&args.name)?;
    let state_dir = crate::runtime::config::omk_state_dir()
        .join(TEAM_DIR)
        .join(&team_name);

    if !state_dir.exists() {
        anyhow::bail!(
            "Team '{}' not found. Expected state at: {}",
            team_name,
            state_dir.display()
        );
    }

    let event_log = state_dir.join(EVENTS_FILE);
    let event_writer = crate::runtime::events::EventWriter::new(&event_log);
    let run_id = crate::runtime::events::RunId(team_name.clone());

    let watchdog = crate::runtime::watchdog::Watchdog::with_defaults();
    let report = watchdog
        .check_team(&run_id, &state_dir, &event_writer)
        .await?;

    println!("🩺 Health check — {}", report.run_id);
    println!("Checked at:  {}", report.checked_at);
    println!("Workers:     {}", report.workers.len());
    println!();

    for worker in &report.workers {
        let status_icon = match worker.status {
            crate::runtime::watchdog::HealthStatus::Healthy => "✅",
            crate::runtime::watchdog::HealthStatus::Stalled => "⚠️",
            crate::runtime::watchdog::HealthStatus::Dead => "❌",
            crate::runtime::watchdog::HealthStatus::Unknown => "❓",
        };
        println!(
            "  {} {:12} inbox={} outbox={}",
            status_icon, worker.worker_id, worker.inbox_count, worker.outbox_count
        );
        println!("     → {}", worker.message);
    }

    println!();

    if report.issues_found == 0 {
        println!("🎉 All workers healthy.");
    } else {
        println!(
            "⚠️  {} issue(s) found. Check events.jsonl for details.",
            report.issues_found
        );
    }

    Ok(())
}

pub(crate) async fn count_jsonl_lines(path: &PathBuf) -> usize {
    if !path.exists() {
        return 0;
    }
    match tokio::fs::read_to_string(path).await {
        Ok(content) => content.lines().filter(|l| !l.trim().is_empty()).count(),
        Err(_) => 0,
    }
}
