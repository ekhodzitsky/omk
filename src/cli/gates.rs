//! Circuit breaker CLI commands.
//!
//! ```text
//! omk gates status --circuit-breakers
//! omk gates reset --gate <name>
//! omk gates reset --all
//! ```

use anyhow::Result;
use chrono::Utc;
use clap::{Parser, Subcommand};

use crate::runtime::gates::circuit_breaker::global_registry;

#[derive(Parser, Debug)]
pub struct Args {
    #[command(subcommand)]
    command: GateCommands,
}

#[derive(Subcommand, Debug)]
enum GateCommands {
    /// Show circuit breaker status for all gates.
    Status {
        /// Filter by gate name (substring match).
        #[arg(short, long)]
        gate: Option<String>,
    },
    /// Reset circuit breaker(s) to Closed.
    Reset {
        /// Specific gate to reset.
        #[arg(short, long)]
        gate: Option<String>,
        /// Reset all gates.
        #[arg(long)]
        all: bool,
    },
}

pub async fn run(args: Args) -> Result<()> {
    match args.command {
        GateCommands::Status { gate } => cmd_status(gate.as_deref()).await,
        GateCommands::Reset { gate, all } => cmd_reset(gate.as_deref(), all).await,
    }
}

async fn cmd_status(gate_filter: Option<&str>) -> Result<()> {
    let registry = global_registry();
    let statuses = registry.list().await;

    if statuses.is_empty() {
        println!("No circuit breakers registered.");
        return Ok(());
    }

    let filtered: Vec<_> = statuses
        .into_iter()
        .filter(|s| gate_filter.map(|f| s.gate_name.contains(f)).unwrap_or(true))
        .collect();

    if filtered.is_empty() {
        println!("No circuit breakers match the filter.");
        return Ok(());
    }

    for s in filtered {
        let state_label = format!("{:?}", s.state).to_uppercase();
        let failures = format!("{}/{}", s.consecutive_failures, s.failure_threshold);

        let detail = match s.state {
            crate::runtime::gates::circuit_breaker::CircuitState::Closed => {
                let last = s
                    .last_success_at
                    .map(|dt| format!("{} ago", format_duration(Utc::now() - dt)))
                    .unwrap_or_else(|| "never".to_string());
                format!("{failures} failures, last success: {last}")
            }
            crate::runtime::gates::circuit_breaker::CircuitState::Open => {
                let opened = s
                    .opened_at
                    .map(|dt| format!("{} ago", format_duration(Utc::now() - dt)))
                    .unwrap_or_else(|| "unknown".to_string());
                let recover_in = s.recovery_timeout_secs.saturating_sub(
                    s.opened_at
                        .map(|dt| (Utc::now() - dt).num_seconds() as u64)
                        .unwrap_or(0),
                );
                format!("{failures} failures, opened: {opened}, recovers in {recover_in}s")
            }
            crate::runtime::gates::circuit_breaker::CircuitState::HalfOpen => {
                let recovered = s
                    .opened_at
                    .map(|dt| format!("{} ago", format_duration(Utc::now() - dt)))
                    .unwrap_or_else(|| "unknown".to_string());
                format!(
                    "{} probe remaining, recovered {recovered}",
                    s.half_open_calls_remaining
                )
            }
        };

        println!("  {}: {} ({})", s.gate_name, state_label, detail);
    }

    Ok(())
}

async fn cmd_reset(gate: Option<&str>, all: bool) -> Result<()> {
    let registry = global_registry();
    let project_dir = std::env::current_dir()?;

    if all {
        registry.reset_all().await?;
        println!("All circuit breakers manually reset to CLOSED.");
        return Ok(());
    }

    if let Some(gate_name) = gate {
        registry.reset(gate_name, &project_dir).await?;
        println!("Circuit breaker for '{gate_name}' manually reset to CLOSED.");
        return Ok(());
    }

    anyhow::bail!("Provide --gate <name> or --all");
}

fn format_duration(d: chrono::Duration) -> String {
    let secs = d.num_seconds();
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else {
        format!("{}h", secs / 3600)
    }
}
