use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
pub(crate) struct Args {
    #[command(subcommand)]
    pub(crate) command: RunCommands,
}

#[derive(Subcommand, Debug)]
pub(crate) enum RunCommands {
    /// List recorded runs
    List,
    /// Show event timeline for a run
    Show {
        /// Run ID or "latest"
        run_id: String,
        /// Output format
        #[arg(short, long, value_enum, default_value = "text")]
        format: OutputFormat,
        /// Filter by event kind
        #[arg(short, long)]
        kind: Option<String>,
    },
}

#[derive(Clone, Debug, clap::ValueEnum)]
pub(crate) enum OutputFormat {
    Text,
    Json,
}

pub(crate) async fn run(args: Args) -> Result<()> {
    match args.command {
        RunCommands::List => cmd_list().await,
        RunCommands::Show {
            run_id,
            format,
            kind,
        } => cmd_show(&run_id, format, kind.as_deref()).await,
    }
}

async fn cmd_list() -> Result<()> {
    let runs_dir = crate::runtime::config::state_dir().join("runs");
    let team_runs_dir =
        crate::runtime::config::omk_state_dir().join(crate::runtime::config::TEAM_DIR);

    let mut runs = vec![];

    // Collect scheduler runs
    if runs_dir.exists() {
        let mut entries = tokio::fs::read_dir(&runs_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            if entry.file_type().await?.is_dir() {
                runs.push((
                    "scheduler".to_string(),
                    entry.file_name().to_string_lossy().to_string(),
                ));
            }
        }
    }

    // Collect team runs
    if team_runs_dir.exists() {
        let mut entries = tokio::fs::read_dir(&team_runs_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            if entry.file_type().await?.is_dir() {
                runs.push((
                    "team".to_string(),
                    entry.file_name().to_string_lossy().to_string(),
                ));
            }
        }
    }

    if runs.is_empty() {
        println!("No runs found.");
        return Ok(());
    }

    // Sort by name descending (names usually contain timestamps)
    runs.sort_by(|a, b| b.1.cmp(&a.1));

    println!("📋 Runs ({}):", runs.len());
    for (mode, name) in &runs {
        println!("  [{:10}] {}", mode, name);
    }

    Ok(())
}

async fn cmd_show(run_id: &str, format: OutputFormat, kind_filter: Option<&str>) -> Result<()> {
    let (state_dir, resolved_run_id) = crate::runtime::state::resolve_run(run_id).await?;
    let event_log = state_dir.join(crate::runtime::config::EVENTS_FILE);
    let events = crate::runtime::events::EventReader::read_all(&event_log).await?;

    let filtered: Vec<_> = if let Some(kind) = kind_filter {
        events
            .into_iter()
            .filter(|e| {
                format!("{:?}", e.kind)
                    .to_lowercase()
                    .contains(&kind.to_lowercase())
            })
            .collect()
    } else {
        events
    };

    match format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&filtered)?);
        }
        OutputFormat::Text => {
            println!(
                "📋 Run timeline — {} ({} events)",
                resolved_run_id,
                filtered.len()
            );
            println!("  Source: {}", event_log.display());
            println!();

            for event in &filtered {
                let actor_str = event.actor.as_deref().unwrap_or("—");
                println!(
                    "  {}  {:22}  actor={}",
                    event.ts.format("%H:%M:%S"),
                    format!("{:?}", event.kind),
                    actor_str
                );
            }
        }
    }

    Ok(())
}
