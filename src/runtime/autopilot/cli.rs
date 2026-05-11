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
) -> Result<()> {
    let mut autopilot = Autopilot::new(name, task, dir, enable_ralph, yolo);

    // Show rough cost estimate
    let rough_estimate = crate::cost::estimator::estimate_autopilot_cost(300, 6);
    println!("  Estimated cost: {}", rough_estimate.formatted());

    let result = autopilot.run().await;

    // Record actual cost
    let duration = chrono::Utc::now()
        .signed_duration_since(autopilot.state.created_at)
        .num_seconds()
        .max(0) as u64;
    let phases = autopilot
        .state
        .execution_log
        .iter()
        .filter(|l| l.success)
        .count();
    let cost = crate::cost::estimator::estimate_autopilot_cost(duration, phases);

    let notification = match &result {
        Ok(_) => crate::notifications::NotificationEvent::AutopilotComplete {
            name: name.to_string(),
            duration_secs: duration,
            phases_completed: phases,
        },
        Err(e) => crate::notifications::NotificationEvent::AutopilotFailed {
            name: name.to_string(),
            phase: format!("{:?}", autopilot.state.phase),
            error: e.to_string(),
        },
    };
    let _ = crate::runtime::session::record_session_end(
        "autopilot",
        name,
        autopilot.state.created_at,
        cost,
        notification,
    )
    .await;

    result
}

/// Resume an existing autopilot run.
pub async fn resume_autopilot(
    name: &str,
    _dir: &Path,
    enable_ralph: bool,
    yolo: bool,
) -> Result<()> {
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

    let result = autopilot.run().await;

    // Record actual cost
    let duration = chrono::Utc::now()
        .signed_duration_since(autopilot.state.created_at)
        .num_seconds()
        .max(0) as u64;
    let phases = autopilot
        .state
        .execution_log
        .iter()
        .filter(|l| l.success)
        .count();
    let cost = crate::cost::estimator::estimate_autopilot_cost(duration, phases);

    let notification = match &result {
        Ok(_) => crate::notifications::NotificationEvent::AutopilotComplete {
            name: name.to_string(),
            duration_secs: duration,
            phases_completed: phases,
        },
        Err(e) => crate::notifications::NotificationEvent::AutopilotFailed {
            name: name.to_string(),
            phase: format!("{:?}", autopilot.state.phase),
            error: e.to_string(),
        },
    };
    let _ = crate::runtime::session::record_session_end(
        "autopilot",
        name,
        autopilot.state.created_at,
        cost,
        notification,
    )
    .await;

    result
}
