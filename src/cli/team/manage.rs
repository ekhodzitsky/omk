use anyhow::{Context, Result};

use crate::cli::team::args::{CleanupArgs, ExportArgs, ImportArgs, RenameArgs, ShutdownArgs};
use crate::cli::team::proof::finalize_team_run_proof;
use crate::runtime::config::{EVENTS_FILE, TEAM_DIR};
use crate::runtime::events::{EventKind, EventWriter, RunId};
use crate::runtime::sanitize::sanitize_name;
use crate::runtime::state::TeamState;
use tracing::warn;

pub(crate) async fn shutdown(args: ShutdownArgs) -> Result<()> {
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

    // Emit manual_interrupt event before killing
    let event_log = state_dir.join(EVENTS_FILE);
    let event_writer = EventWriter::new(&event_log);
    let run_id = RunId(team_name.clone());
    let interrupt_event =
        crate::runtime::events::Event::new(run_id.clone(), EventKind::ManualInterrupt)
            .with_actor("omk-cli");
    let _ = event_writer.append(&interrupt_event).await;

    if !args.force {
        println!("Marking team '{}' as interrupted...", team_name);
    }

    // Update state
    let mut state = TeamState::load(&state_dir).await?;
    state.phase = crate::runtime::state::TeamPhase::Shutdown;
    state.save().await?;

    if let Err(e) = finalize_team_run_proof(&state_dir, &event_writer, &run_id).await {
        warn!(error = %e, team = %team_name, "Failed to write shutdown proof artifact");
    }

    // Estimate and record cost
    let duration = chrono::Utc::now().signed_duration_since(state.created_at);
    let cost_estimate = crate::cost::estimator::estimate_team_cost(
        u64::try_from(duration.num_seconds()).unwrap_or(0),
        state.worker_count,
        &state.worker_role,
    );

    let summary = crate::runtime::session::SessionSummary {
        session_type: "team".to_string(),
        name: team_name.clone(),
        started_at: state.created_at,
        ended_at: chrono::Utc::now(),
        duration_secs: u64::try_from(duration.num_seconds()).unwrap_or(0),
        jobs_total: None,
        jobs_success: None,
        phases_completed: None,
        iterations: None,
        verified: None,
        total_stories: None,
    };
    let _ = crate::cli::session::record_session_end(
        &summary,
        cost_estimate.clone(),
        crate::notifications::NotificationEvent::TeamShutdown {
            name: team_name.clone(),
            duration_secs: summary.duration_secs,
            status: if args.force { "forced" } else { "graceful" }.to_string(),
        },
    )
    .await;

    // Record metrics
    let _ = crate::runtime::metrics::record(
        &crate::runtime::config::state_dir().join("metrics.json"),
        |m| m.total_shutdowns += 1,
    )
    .await;

    println!("✓ Team '{}' shut down", team_name);
    println!("  State:   {}", state_dir.display());
    println!("  Cost:    {}", cost_estimate.formatted());

    Ok(())
}

pub(crate) async fn cleanup(args: CleanupArgs) -> Result<()> {
    let teams_dir = crate::runtime::config::omk_state_dir().join(TEAM_DIR);
    let older_than = if args.all {
        None
    } else {
        Some(args.older_than)
    };
    let (removed, freed) =
        crate::cli::cleanup::cleanup_team_states(&teams_dir, older_than, args.dry_run).await?;
    println!();
    if args.dry_run {
        println!(
            "Would remove {removed} team state directories ({:.1} MB)",
            freed as f64 / 1_048_576.0
        );
    } else {
        println!(
            "Removed {removed} team state directories ({:.1} MB freed)",
            freed as f64 / 1_048_576.0
        );
    }
    Ok(())
}

pub(crate) async fn rename_team(args: RenameArgs) -> Result<()> {
    let old_name = sanitize_name(&args.old_name)?;
    let new_name = sanitize_name(&args.new_name)?;
    let state_dir = crate::runtime::config::omk_state_dir().join(TEAM_DIR);
    let old_path = state_dir.join(&old_name);
    let new_path = state_dir.join(&new_name);

    if !old_path.exists() {
        anyhow::bail!("Team '{}' not found", old_name);
    }

    if new_path.exists() {
        anyhow::bail!("Team '{}' already exists", new_name);
    }

    tokio::fs::rename(&old_path, &new_path).await?;

    // Update state file
    let state_file = new_path.join("team-state.json");
    if let Ok(content) = tokio::fs::read_to_string(&state_file).await {
        if let Ok(mut state) = serde_json::from_str::<crate::runtime::state::TeamState>(&content) {
            state.name = new_name.clone();
            state.save().await?;
        }
    }

    println!("✓ Renamed team '{}' → '{}'", old_name, new_name);
    Ok(())
}

pub(crate) async fn export_team(args: ExportArgs) -> Result<()> {
    let team_name = sanitize_name(&args.name)?;
    let state_dir = crate::runtime::config::omk_state_dir()
        .join(TEAM_DIR)
        .join(&team_name);
    let state_file = state_dir.join("team-state.json");

    if !state_file.exists() {
        anyhow::bail!("Team '{}' not found", team_name);
    }

    let content = tokio::fs::read_to_string(&state_file).await?;
    let state: crate::runtime::state::TeamState = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse team state for '{}'", team_name))?;

    let export = serde_json::json!({
        "version": "1.0",
        "exported_at": chrono::Utc::now().to_rfc3339(),
        "team": state,
    });

    let json = serde_json::to_string_pretty(&export)?;
    crate::runtime::atomic::atomic_write(std::path::Path::new(&args.output), json.as_bytes())
        .await?;

    println!("✓ Exported team '{}' to {}", team_name, args.output);
    Ok(())
}

pub(crate) async fn import_team(args: ImportArgs) -> Result<()> {
    let content = tokio::fs::read_to_string(&args.file)
        .await
        .with_context(|| format!("Failed to read file '{}'", args.file))?;
    let export: serde_json::Value = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse JSON from '{}'", args.file))?;

    let team_value = export
        .get("team")
        .ok_or_else(|| anyhow::anyhow!("Invalid export file: missing 'team' field"))?;
    let state: crate::runtime::state::TeamState = serde_json::from_value(team_value.clone())
        .with_context(|| "Failed to deserialize team state")?;

    let state_dir = crate::runtime::config::omk_state_dir()
        .join(TEAM_DIR)
        .join(&state.name);
    tokio::fs::create_dir_all(&state_dir).await?;

    state.save().await?;

    println!("✓ Imported team '{}' from {}", state.name, args.file);
    println!("  State dir: {}", state_dir.display());
    println!("  Run `omk team status {}` to inspect it", state.name);
    Ok(())
}
