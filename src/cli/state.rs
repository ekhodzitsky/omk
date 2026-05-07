use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use serde_json::Value;
use std::path::Path;
use tracing::info;

#[derive(Parser, Debug)]
pub struct Args {
    #[command(subcommand)]
    command: StateCommands,
}

#[derive(Subcommand, Debug)]
pub enum StateCommands {
    /// Export all state to a single JSON file
    Export {
        /// Output file path
        #[arg(short, long, default_value = "omk-state-export.json")]
        output: String,
    },
    /// Import state from a JSON file
    Import {
        /// Input file path
        #[arg(short, long)]
        input: String,
    },
}

pub async fn run(args: Args) -> Result<()> {
    match args.command {
        StateCommands::Export { output } => export_state(&output).await,
        StateCommands::Import { input } => import_state(&input).await,
    }
}

async fn export_state(output: &str) -> Result<()> {
    let state_dir = crate::runtime::config::state_dir();

    let mut export = serde_json::json!({
        "version": "1.0",
        "exported_at": chrono::Utc::now().to_rfc3339(),
        "teams": [],
        "autopilots": [],
        "ralphs": [],
        "metrics": null,
    });

    // Export team states
    let teams_dir = state_dir.join("team");
    if teams_dir.exists() {
        let mut teams = Vec::new();
        let mut entries = tokio::fs::read_dir(&teams_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let team_state = entry.path().join("team-state.json");
            if team_state.exists() {
                let content = tokio::fs::read_to_string(&team_state).await?;
                let value: Value = serde_json::from_str(&content)
                    .with_context(|| format!("parse {}", team_state.display()))?;
                teams.push(value);
            }
        }
        export["teams"] = serde_json::Value::Array(teams);
    }

    // Export autopilot states
    let autopilot_dir = state_dir.join("autopilot");
    if autopilot_dir.exists() {
        let mut autopilots = Vec::new();
        let mut entries = tokio::fs::read_dir(&autopilot_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let ap_state = entry.path().join("autopilot-state.json");
            if ap_state.exists() {
                let content = tokio::fs::read_to_string(&ap_state).await?;
                let value: Value = serde_json::from_str(&content)
                    .with_context(|| format!("parse {}", ap_state.display()))?;
                autopilots.push(value);
            }
        }
        export["autopilots"] = serde_json::Value::Array(autopilots);
    }

    // Export ralph states
    let ralph_dir = state_dir.join("ralph");
    if ralph_dir.exists() {
        let mut ralphs = Vec::new();
        let mut entries = tokio::fs::read_dir(&ralph_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let ralph_state = entry.path().join("ralph-state.json");
            if ralph_state.exists() {
                let content = tokio::fs::read_to_string(&ralph_state).await?;
                let value: Value = serde_json::from_str(&content)
                    .with_context(|| format!("parse {}", ralph_state.display()))?;
                ralphs.push(value);
            }
        }
        export["ralphs"] = serde_json::Value::Array(ralphs);
    }

    // Export metrics
    let metrics_path = state_dir.join("metrics.json");
    if metrics_path.exists() {
        let content = tokio::fs::read_to_string(&metrics_path).await?;
        let value: Value = serde_json::from_str(&content)?;
        export["metrics"] = value;
    }

    let json = serde_json::to_string_pretty(&export)?;
    crate::runtime::atomic::atomic_write(Path::new(output), json.as_bytes()).await?;

    info!(path = %output, "Exported state");
    println!("✓ State exported to {}", output);
    println!("  Teams:      {}", export["teams"].as_array().map(|a| a.len()).unwrap_or(0));
    println!("  Autopilots: {}", export["autopilots"].as_array().map(|a| a.len()).unwrap_or(0));
    println!("  Ralphs:     {}", export["ralphs"].as_array().map(|a| a.len()).unwrap_or(0));

    Ok(())
}

async fn import_state(input: &str) -> Result<()> {
    let content = tokio::fs::read_to_string(input).await?;
    let export: Value = serde_json::from_str(&content)
        .with_context(|| format!("parse {}", input))?;

    println!("Importing state from {}...", input);

    let state_dir = crate::runtime::config::state_dir();

    // Import teams
    if let Some(teams) = export["teams"].as_array() {
        for team in teams {
            if let Some(name) = team["name"].as_str() {
                let team_dir = state_dir.join("team").join(name);
                tokio::fs::create_dir_all(&team_dir).await?;
                let path = team_dir.join("team-state.json");
                let json = serde_json::to_vec_pretty(team)?;
                crate::runtime::atomic::atomic_write(&path, &json).await?;
                println!("  ✓ Imported team: {}", name);
            }
        }
    }

    // Import autopilots
    if let Some(autopilots) = export["autopilots"].as_array() {
        for ap in autopilots {
            if let Some(name) = ap["name"].as_str().or_else(|| ap["task"].as_str()) {
                let ap_dir = state_dir.join("autopilot").join(name);
                tokio::fs::create_dir_all(&ap_dir).await?;
                let path = ap_dir.join("autopilot-state.json");
                let json = serde_json::to_vec_pretty(ap)?;
                crate::runtime::atomic::atomic_write(&path, &json).await?;
                println!("  ✓ Imported autopilot: {}", name);
            }
        }
    }

    // Import ralphs
    if let Some(ralphs) = export["ralphs"].as_array() {
        for ralph in ralphs {
            if let Some(task) = ralph["task"].as_str() {
                let slug = task.split_whitespace().take(5).collect::<Vec<_>>().join("-").to_lowercase();
                let ralph_dir = state_dir.join("ralph").join(&slug);
                tokio::fs::create_dir_all(&ralph_dir).await?;
                let path = ralph_dir.join("ralph-state.json");
                let json = serde_json::to_vec_pretty(ralph)?;
                crate::runtime::atomic::atomic_write(&path, &json).await?;
                println!("  ✓ Imported ralph: {}", slug);
            }
        }
    }

    // Import metrics
    if let Some(metrics) = export.get("metrics") {
        let path = state_dir.join("metrics.json");
        let json = serde_json::to_vec_pretty(metrics)?;
        crate::runtime::atomic::atomic_write(&path, &json).await?;
        println!("  ✓ Imported metrics");
    }

    println!("✓ State import complete");
    Ok(())
}
