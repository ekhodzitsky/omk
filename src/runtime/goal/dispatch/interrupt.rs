use std::time::Duration;
use tokio_util::sync::CancellationToken;

use crate::runtime::goal::state::{FileSystemGoalStateStore, GoalStateStore, GoalStatus};

pub(crate) async fn watch_goal_control_interrupt(
    goal_dir: std::path::PathBuf,
    worker_cancel: CancellationToken,
    monitor_cancel: CancellationToken,
) -> Option<GoalStatus> {
    loop {
        tokio::select! {
            biased;
            _ = monitor_cancel.cancelled() => return None,
            _ = tokio::time::sleep(goal_interrupt_poll_interval()) => {
                let Ok(state) = FileSystemGoalStateStore::new().load(&goal_dir).await else {
                    continue;
                };
                if matches!(state.status, GoalStatus::Paused | GoalStatus::Cancelled) {
                    worker_cancel.cancel();
                    return Some(state.status);
                }
            }
        }
    }
}

fn goal_interrupt_poll_interval() -> Duration {
    std::env::var("OMK_GOAL_INTERRUPT_POLL_MS")
        .ok()
        .and_then(|raw| raw.trim().parse::<u64>().ok())
        .filter(|millis| *millis > 0)
        .map(Duration::from_millis)
        .unwrap_or_else(|| Duration::from_millis(500))
}
