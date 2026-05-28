use anyhow::Result;
use std::path::Path;
use tracing::info;

use crate::runtime::autopilot::engine::Autopilot;

/// Convenience entry-point used by the CLI.
pub async fn run_autopilot(
    name: &str,
    task: &str,
    dir: &Path,
    enable_ralph: bool,
    yolo: bool,
) -> Result<crate::runtime::session::SessionSummary> {
    let mut autopilot = Autopilot::new(name, task, dir, enable_ralph, yolo);

    autopilot.run().await?;

    let duration = u64::try_from(
        chrono::Utc::now()
            .signed_duration_since(autopilot.state.created_at)
            .num_seconds(),
    )
    .unwrap_or(0);
    let phases = autopilot
        .state
        .execution_log
        .iter()
        .filter(|l| l.success)
        .count();

    Ok(crate::runtime::session::SessionSummary {
        session_type: "autopilot".to_string(),
        name: name.to_string(),
        started_at: autopilot.state.created_at,
        ended_at: chrono::Utc::now(),
        duration_secs: duration,
        jobs_total: None,
        jobs_success: None,
        phases_completed: Some(phases),
        iterations: None,
        verified: None,
        total_stories: None,
    })
}

/// Resume an existing autopilot run.
pub async fn resume_autopilot(
    name: &str,
    _dir: &Path,
    enable_ralph: bool,
    yolo: bool,
) -> Result<crate::runtime::session::SessionSummary> {
    let state_dir = crate::runtime::config::state_dir()
        .join("autopilot")
        .join(name);
    if !state_dir.exists() {
        anyhow::bail!(
            "Autopilot run '{}' not found at {}",
            name,
            state_dir.display()
        );
    }

    let mut autopilot = Autopilot::from_state(&state_dir, enable_ralph, yolo).await?;
    info!(name = %name, phase = ?autopilot.state.phase, "Resuming autopilot");

    autopilot.run().await?;

    let duration = u64::try_from(
        chrono::Utc::now()
            .signed_duration_since(autopilot.state.created_at)
            .num_seconds(),
    )
    .unwrap_or(0);
    let phases = autopilot
        .state
        .execution_log
        .iter()
        .filter(|l| l.success)
        .count();

    Ok(crate::runtime::session::SessionSummary {
        session_type: "autopilot".to_string(),
        name: name.to_string(),
        started_at: autopilot.state.created_at,
        ended_at: chrono::Utc::now(),
        duration_secs: duration,
        jobs_total: None,
        jobs_success: None,
        phases_completed: Some(phases),
        iterations: None,
        verified: None,
        total_stories: None,
    })
}
