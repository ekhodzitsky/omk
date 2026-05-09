use anyhow::Result;
use clap::{Parser, Subcommand};
use serde_json::Value;

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
        /// Output JSON (shortcut for --format json)
        #[arg(long)]
        json: bool,
        /// Filter by event kind
        #[arg(short, long)]
        kind: Option<String>,
        /// Filter by worker id
        #[arg(long)]
        worker: Option<String>,
        /// Filter by task id
        #[arg(long)]
        task: Option<String>,
    },
}

#[derive(Copy, Clone, Debug, clap::ValueEnum)]
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
            json,
            kind,
            worker,
            task,
        } => {
            cmd_show(
                &run_id,
                format,
                json,
                kind.as_deref(),
                worker.as_deref(),
                task.as_deref(),
            )
            .await
        }
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

async fn cmd_show(
    run_id: &str,
    format: OutputFormat,
    json: bool,
    kind_filter: Option<&str>,
    worker_filter: Option<&str>,
    task_filter: Option<&str>,
) -> Result<()> {
    let (state_dir, resolved_run_id) = crate::runtime::state::resolve_run(run_id).await?;
    let event_log = state_dir.join(crate::runtime::config::EVENTS_FILE);
    let mut used_worker_reader = false;
    let mut used_task_reader = false;
    let events = if let Some(worker) = worker_filter {
        used_worker_reader = true;
        crate::runtime::events::EventReader::read_for_worker(&event_log, worker).await?
    } else if let Some(task_id) = task_filter {
        used_task_reader = true;
        crate::runtime::events::EventReader::read_for_task(&event_log, task_id).await?
    } else {
        crate::runtime::events::EventReader::read_all(&event_log).await?
    };

    let kind_filter_lc = kind_filter.map(str::to_lowercase);
    let filtered: Vec<_> = events
        .into_iter()
        .filter(|event| {
            if !used_worker_reader {
                if let Some(worker) = worker_filter {
                    if event.actor.as_deref() != Some(worker) {
                        return false;
                    }
                }
            }
            if !used_task_reader {
                if let Some(task_id) = task_filter {
                    if payload_string(event, "task_id").as_deref() != Some(task_id) {
                        return false;
                    }
                }
            }
            if let Some(kind) = &kind_filter_lc {
                if !event_kind_name(event).to_lowercase().contains(kind) {
                    return false;
                }
            }
            true
        })
        .collect();

    let output_format = if json { OutputFormat::Json } else { format };

    match output_format {
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
                let task_col = payload_string(event, "task_id")
                    .map(|task_id| format!("  task={task_id}"))
                    .unwrap_or_default();
                let evidence_col = event_wire_evidence_columns(event)
                    .map(|details| format!("  {details}"))
                    .unwrap_or_default();
                println!(
                    "  {}  {:22}  actor={}{}{}",
                    event.ts.format("%H:%M:%S"),
                    event_kind_name(event),
                    actor_str,
                    task_col,
                    evidence_col
                );
            }
        }
    }

    Ok(())
}

fn event_kind_name(event: &crate::runtime::events::Event) -> String {
    serde_json::to_value(&event.kind)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| format!("{:?}", event.kind))
}

fn payload_string(event: &crate::runtime::events::Event, key: &str) -> Option<String> {
    event
        .payload
        .as_ref()
        .and_then(|payload| payload.get(key))
        .and_then(value_as_string)
}

fn value_as_string(value: &Value) -> Option<String> {
    if let Some(text) = value.as_str() {
        return Some(text.to_string());
    }
    value.get("0")?.as_str().map(str::to_string)
}

fn payload_field_string(
    event: &crate::runtime::events::Event,
    key: &'static str,
    aliases: &[&'static str],
) -> Option<String> {
    let payload = event.payload.as_ref()?;
    let raw = payload
        .get(key)
        .or_else(|| aliases.iter().find_map(|alias| payload.get(alias)))?;
    value_as_string(raw).map(|value| sanitize_inline(&value))
}

fn sanitize_inline(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn event_wire_evidence_columns(event: &crate::runtime::events::Event) -> Option<String> {
    let fields: [(&str, &[&str]); 7] = [
        ("wire_method", &["method"]),
        ("wire_event", &["event_type", "type"]),
        ("wire_request", &["request_type", "raw_request_type"]),
        ("wire_request_id", &["request_id"]),
        ("output_summary", &[]),
        ("message", &[]),
        ("reason", &["error"]),
    ];

    let details: Vec<String> = fields
        .iter()
        .filter_map(|(key, aliases)| {
            payload_field_string(event, key, aliases)
                .filter(|value| !value.is_empty())
                .map(|value| format!("{key}={value}"))
        })
        .collect();

    if details.is_empty() {
        None
    } else {
        Some(details.join("  "))
    }
}
