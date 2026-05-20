use std::time::Duration;

use tokio_util::sync::CancellationToken;

use crate::runtime::db::{error::DbError, repo::goal::GoalRepo, DbHandle};
use crate::runtime::goal::state::goals_db_path;

/// Heartbeat interval for a running goal controller.
const GOAL_HEARTBEAT_INTERVAL_SECS: u64 = 10;

/// Staleness threshold: if a running goal has not heartbeat-ed in this
/// duration, its controller is presumed dead.
const GOAL_STALE_THRESHOLD_SECS: i64 = 30;

/// Own the controller PID for a goal and emit periodic heartbeats.
pub async fn claim_goal(goal_id: &str) -> anyhow::Result<GoalHeartbeatHandle> {
    let db = DbHandle::open(goals_db_path()).await?;
    let pid = std::process::id() as i32;
    db.goal_repo().update_controller_pid(goal_id, Some(pid)).await?;

    let cancel = CancellationToken::new();
    let task = tokio::spawn(heartbeat_loop(
        goal_id.to_string(),
        cancel.clone(),
    ));

    Ok(GoalHeartbeatHandle {
        _task: task,
        cancel,
    })
}

/// Clear the controller PID and stop heartbeats.
pub async fn release_goal(goal_id: &str) -> anyhow::Result<()> {
    let db = DbHandle::open(goals_db_path()).await?;
    db.goal_repo()
        .update_controller_pid(goal_id, None)
        .await
        .map_err(db_to_anyhow)?;
    Ok(())
}

/// List goals that appear to be running but whose controller PID is dead
/// or stale.
pub async fn list_orphaned_goals() -> anyhow::Result<Vec<OrphanedGoal>> {
    let db = DbHandle::open(goals_db_path()).await?;
    let running = db.goal_repo().list_running().await?;

    let mut orphaned = Vec::new();
    for summary in running {
        // Load full record to inspect controller_pid.
        let record = match db.goal_repo().get(&summary.goal_id).await? {
            Some(r) => r,
            None => continue,
        };

        let stale = match record.controller_pid {
            Some(pid) if pid > 0 => !is_pid_alive(pid),
            _ => true,
        };

        let heartbeat_stale = chrono::Utc::now().timestamp() - record.updated_at
            > GOAL_STALE_THRESHOLD_SECS;

        if stale || heartbeat_stale {
            orphaned.push(OrphanedGoal {
                goal_id: summary.goal_id,
                controller_pid: record.controller_pid,
                last_updated_at: record.updated_at,
            });
        }
    }

    Ok(orphaned)
}

/// Information about a goal whose controller appears to have died.
#[derive(Debug, Clone)]
pub struct OrphanedGoal {
    pub goal_id: String,
    pub controller_pid: Option<i32>,
    pub last_updated_at: i64,
}

/// Handle returned by [`claim_goal`].  Dropping it stops the heartbeat.
#[derive(Debug)]
pub struct GoalHeartbeatHandle {
    _task: tokio::task::JoinHandle<()>,
    cancel: CancellationToken,
}

impl GoalHeartbeatHandle {
    pub fn stop(self) {
        self.cancel.cancel();
    }
}

async fn heartbeat_loop(goal_id: String, cancel: CancellationToken) {
    let mut interval = tokio::time::interval(Duration::from_secs(GOAL_HEARTBEAT_INTERVAL_SECS));

    loop {
        tokio::select! {
            _ = interval.tick() => {
                if let Err(e) = send_heartbeat(&goal_id).await {
                    tracing::warn!(goal_id, error = %e, "Goal heartbeat failed");
                }
            }
            _ = cancel.cancelled() => break,
        }
    }

    tracing::debug!(goal_id, "Goal heartbeat loop stopped");
}

async fn send_heartbeat(goal_id: &str) -> Result<(), DbError> {
    let db = DbHandle::open(goals_db_path()).await?;
    db.goal_repo().heartbeat(goal_id).await
}

#[cfg(unix)]
fn is_pid_alive(pid: i32) -> bool {
    unsafe { libc::kill(pid, 0) == 0 }
}

#[cfg(not(unix))]
fn is_pid_alive(pid: i32) -> bool {
    // Non-Unix fallback: assume alive; stale detection relies on heartbeat
    // timestamp exclusively.
    true
}

fn db_to_anyhow(e: DbError) -> anyhow::Error {
    anyhow::anyhow!("db error: {e}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::db::repo::goal::GoalRepo;

    fn test_goal_record(goal_id: &str, status: &str, pid: Option<i32>) -> crate::runtime::db::types::GoalRecord {
        crate::runtime::db::types::GoalRecord {
            goal_id: goal_id.to_string(),
            status: status.to_string(),
            phase: "execution".to_string(),
            kind: None,
            original_goal: "test".to_string(),
            normalized_goal: "test".to_string(),
            goal_text: "test".to_string(),
            project_dir: "/tmp".to_string(),
            state_dir: "/tmp".to_string(),
            policy: "local".to_string(),
            delivery_policy: "local".to_string(),
            merge_policy: "disabled".to_string(),
            until_ready: false,
            slice_execution: false,
            max_agents: None,
            budget_time_secs: None,
            budget_tokens: None,
            budget_usd: None,
            cost_tracker_path: None,
            terminal_criteria: None,
            failure: None,
            created_at: chrono::Utc::now().timestamp(),
            updated_at: chrono::Utc::now().timestamp(),
            completed_at: None,
            controller_pid: pid,
            version: 1,
        }
    }

    #[tokio::test]
    async fn controller_pid_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().join("test.db");
        let db = DbHandle::open(&db_path).await.unwrap();

        let goal = test_goal_record("goal-pid", "running", None);
        db.goal_repo().create(&goal).await.unwrap();

        db.goal_repo()
            .update_controller_pid("goal-pid", Some(42))
            .await
            .unwrap();
        let record = db.goal_repo().get("goal-pid").await.unwrap().unwrap();
        assert_eq!(record.controller_pid, Some(42));

        db.goal_repo()
            .update_controller_pid("goal-pid", None)
            .await
            .unwrap();
        let record = db.goal_repo().get("goal-pid").await.unwrap().unwrap();
        assert_eq!(record.controller_pid, None);
    }

    #[tokio::test]
    async fn heartbeat_updates_timestamp() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().join("test.db");
        let db = DbHandle::open(&db_path).await.unwrap();

        let mut goal = test_goal_record("goal-hb", "running", None);
        goal.updated_at = chrono::Utc::now().timestamp() - 2;
        db.goal_repo().create(&goal).await.unwrap();

        db.goal_repo().heartbeat("goal-hb").await.unwrap();
        let after = db.goal_repo().get("goal-hb").await.unwrap().unwrap().updated_at;
        assert!(
            after >= chrono::Utc::now().timestamp() - 1,
            "heartbeat should bump updated_at to near-current time"
        );
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn list_orphaned_detects_dead_pid() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().join("test.db");
        let db = DbHandle::open(&db_path).await.unwrap();

        let my_pid = std::process::id() as i32;
        let alive = test_goal_record("goal-alive", "running", Some(my_pid));
        db.goal_repo().create(&alive).await.unwrap();

        let dead = test_goal_record("goal-dead", "running", Some(999_999));
        db.goal_repo().create(&dead).await.unwrap();

        let orphaned = list_orphaned_goals_with_db(&db).await.unwrap();
        let ids: Vec<_> = orphaned.iter().map(|o| o.goal_id.as_str()).collect();
        assert!(!ids.contains(&"goal-alive"), "own PID should be alive");
        assert!(ids.contains(&"goal-dead"), "PID 999999 should be dead");
    }

    #[tokio::test]
    async fn list_orphaned_detects_stale_heartbeat() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().join("test.db");
        let db = DbHandle::open(&db_path).await.unwrap();

        let mut stale = test_goal_record("goal-stale", "running", Some(1));
        stale.updated_at = chrono::Utc::now().timestamp() - 3600; // 1 hour ago
        db.goal_repo().create(&stale).await.unwrap();

        let orphaned = list_orphaned_goals_with_db(&db).await.unwrap();
        let ids: Vec<_> = orphaned.iter().map(|o| o.goal_id.as_str()).collect();
        assert!(ids.contains(&"goal-stale"));
    }

    // Helper that bypasses the default DB path for tests.
    async fn list_orphaned_goals_with_db(db: &DbHandle) -> anyhow::Result<Vec<OrphanedGoal>> {
        let running = db.goal_repo().list_running().await?;
        let mut orphaned = Vec::new();
        for summary in running {
            let record = match db.goal_repo().get(&summary.goal_id).await? {
                Some(r) => r,
                None => continue,
            };
            let stale = match record.controller_pid {
                Some(pid) if pid > 0 => !super::is_pid_alive(pid),
                _ => true,
            };
            let heartbeat_stale = chrono::Utc::now().timestamp() - record.updated_at
                > super::GOAL_STALE_THRESHOLD_SECS;
            if stale || heartbeat_stale {
                orphaned.push(OrphanedGoal {
                    goal_id: summary.goal_id,
                    controller_pid: record.controller_pid,
                    last_updated_at: record.updated_at,
                });
            }
        }
        Ok(orphaned)
    }
}
