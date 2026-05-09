use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Mutex;
use tracing::{info, warn};

use super::bridge::TeamBridge;
use super::config::WORKERS_DIR;
use super::events::{Event, EventKind, EventWriter, RunId};
use super::scheduler::worker_state::{WorkerState, WorkerStateMap};
use super::worker::WorkerSpec;

/// Default heartbeat missing threshold in seconds.
pub const HEARTBEAT_INTERVAL_SECS: u64 = 30;

/// Configuration for heartbeat and stall detection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchdogConfig {
    /// Seconds without any heartbeat before a worker is considered missing.
    pub heartbeat_missing_secs: u64,
    /// Seconds without heartbeat content change before a worker is considered stalled.
    pub heartbeat_stale_secs: u64,
    /// Seconds a kimi command can run before it is considered stuck.
    pub command_timeout_secs: u64,
    /// Whether to attempt recovery when a worker is dead.
    pub attempt_recovery: bool,
    /// Whether to require a tmux session for workers to be considered healthy.
    #[serde(default = "default_require_tmux")]
    pub require_tmux: bool,
}

fn default_require_tmux() -> bool {
    true
}

impl Default for WatchdogConfig {
    fn default() -> Self {
        Self {
            heartbeat_missing_secs: HEARTBEAT_INTERVAL_SECS,
            heartbeat_stale_secs: 60,
            command_timeout_secs: 300,
            attempt_recovery: false,
            require_tmux: true,
        }
    }
}

/// Health check result for a single worker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerHealth {
    pub worker_id: String,
    pub status: HealthStatus,
    pub last_heartbeat: Option<DateTime<Utc>>,
    pub heartbeat_content: Option<String>,
    pub tmux_pane_alive: bool,
    pub inbox_count: usize,
    pub outbox_count: usize,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthStatus {
    Healthy,
    Stalled,
    Dead,
    Unknown,
}

/// Run-level health report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthReport {
    pub run_id: String,
    pub checked_at: DateTime<Utc>,
    pub tmux_session_alive: bool,
    pub workers: Vec<WorkerHealth>,
    pub issues_found: usize,
}

/// Result of a recovery attempt.
#[derive(Debug, Clone)]
pub struct RecoveryResult {
    pub worker_id: String,
    pub action: String,
    pub success: bool,
    pub message: String,
}

/// Watchdog checks worker health and records events.
pub struct Watchdog {
    config: WatchdogConfig,
    worker_states: Mutex<WorkerStateMap>,
}

impl Watchdog {
    pub fn new(config: WatchdogConfig) -> Self {
        Self {
            config,
            worker_states: Mutex::new(WorkerStateMap::default()),
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(WatchdogConfig::default())
    }

    /// Check health for all workers in a team run and emit events for issues.
    pub async fn check_team(
        &self,
        run_id: &RunId,
        state_dir: &Path,
        event_writer: &EventWriter,
    ) -> Result<HealthReport> {
        self.check_team_inner(run_id, state_dir, Some(event_writer))
            .await
    }

    /// Check health for all workers without writing events (read-only / HUD use).
    pub async fn check_team_read_only(
        &self,
        run_id: &RunId,
        state_dir: &Path,
    ) -> Result<HealthReport> {
        self.check_team_inner(run_id, state_dir, None).await
    }

    async fn check_team_inner(
        &self,
        run_id: &RunId,
        state_dir: &Path,
        event_writer: Option<&EventWriter>,
    ) -> Result<HealthReport> {
        let team_name = &run_id.0;
        let session_name = format!("omk-team-{team_name}");
        let tmux_alive = crate::runtime::tmux::session_exists(&session_name).unwrap_or(false);

        let mut workers = Vec::new();
        let mut issues_found = 0;

        let workers_dir = state_dir.join(WORKERS_DIR);
        if workers_dir.exists() {
            let mut entries = tokio::fs::read_dir(&workers_dir).await?;
            while let Some(entry) = entries.next_entry().await? {
                let worker_dir = entry.path();
                let spec = match WorkerSpec::load(&worker_dir).await {
                    Ok(s) => s,
                    Err(e) => {
                        warn!(dir = %worker_dir.display(), error = %e, "Failed to load worker spec");
                        continue;
                    }
                };

                let health = self.check_worker(&spec, tmux_alive).await?;

                let new_state = match health.status {
                    HealthStatus::Healthy => {
                        let current = self.worker_states.lock().unwrap().get(&spec.name);
                        match current {
                            WorkerState::Stalled | WorkerState::Dead => WorkerState::Ready,
                            _ => current,
                        }
                    }
                    HealthStatus::Stalled => WorkerState::Stalled,
                    HealthStatus::Dead => WorkerState::Dead,
                    HealthStatus::Unknown => WorkerState::Starting,
                };

                let event_to_emit = {
                    let mut states = self.worker_states.lock().unwrap();
                    states
                        .transition(&spec.name, new_state)
                        .and_then(|old_state| {
                            if new_state == WorkerState::Stalled || new_state == WorkerState::Dead {
                                let event_kind = if new_state == WorkerState::Stalled {
                                    EventKind::WorkerStalled
                                } else {
                                    EventKind::WorkerDead
                                };
                                let event = Event::new(run_id.clone(), event_kind.clone())
                                    .with_actor(&spec.name)
                                    .with_message(format!(
                                        "Worker state transitioned from {:?} to {:?}: {}",
                                        old_state, new_state, health.message
                                    ))
                                    .unwrap_or_else(|_| {
                                        Event::new(run_id.clone(), event_kind)
                                            .with_actor(&spec.name)
                                    });
                                Some(event)
                            } else {
                                None
                            }
                        })
                };

                if let Some(event) = event_to_emit {
                    if let Some(ew) = event_writer {
                        let _ = ew.append(&event).await;
                    }
                }

                if health.status != HealthStatus::Healthy {
                    issues_found += 1;
                }

                workers.push(health);
            }
        }

        let report = HealthReport {
            run_id: team_name.clone(),
            checked_at: Utc::now(),
            tmux_session_alive: tmux_alive,
            workers,
            issues_found,
        };

        if issues_found > 0 {
            warn!(run = %team_name, issues = issues_found, "Watchdog detected issues");
        } else {
            info!(run = %team_name, "Watchdog check passed");
        }

        Ok(report)
    }

    async fn check_worker(&self, spec: &WorkerSpec, tmux_alive: bool) -> Result<WorkerHealth> {
        let now = Utc::now();
        let mut last_heartbeat = None;
        let mut heartbeat_content = None;
        let (mut status, mut message);

        // Read heartbeat
        if spec.heartbeat.exists() {
            match tokio::fs::read_to_string(&spec.heartbeat).await {
                Ok(json) => {
                    heartbeat_content = Some(json.clone());
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&json) {
                        if let Some(ts_str) = v.get("ts").and_then(|s| s.as_str()) {
                            if let Ok(ts) = DateTime::parse_from_rfc3339(ts_str) {
                                last_heartbeat = Some(ts.with_timezone(&Utc));
                            }
                        }

                        // Check heartbeat freshness
                        if let Some(ts) = last_heartbeat {
                            let age_secs =
                                now.signed_duration_since(ts).num_seconds().max(0) as u64;
                            if age_secs > self.config.heartbeat_missing_secs {
                                status = HealthStatus::Dead;
                                message = format!(
                                    "No heartbeat for {}s (threshold: {}s)",
                                    age_secs, self.config.heartbeat_missing_secs
                                );
                            } else if age_secs > self.config.heartbeat_stale_secs {
                                status = HealthStatus::Stalled;
                                message = format!(
                                    "Stale heartbeat for {}s (threshold: {}s)",
                                    age_secs, self.config.heartbeat_stale_secs
                                );
                            } else {
                                status = HealthStatus::Healthy;
                                message = format!("Heartbeat fresh ({}s ago)", age_secs);
                            }
                        } else {
                            status = HealthStatus::Stalled;
                            message = "Heartbeat has no timestamp".to_string();
                        }
                    } else {
                        status = HealthStatus::Stalled;
                        message = "Invalid heartbeat JSON".to_string();
                    }
                }
                Err(e) => {
                    status = HealthStatus::Dead;
                    message = format!("Cannot read heartbeat: {}", e);
                }
            }
        } else {
            status = HealthStatus::Dead;
            message = "Heartbeat file missing".to_string();
        }

        // Check tmux pane (coarse check: if session is dead, all panes are dead)
        let tmux_pane_alive = tmux_alive;
        if !tmux_alive && self.config.require_tmux && status != HealthStatus::Dead {
            status = HealthStatus::Dead;
            message = format!("{}; tmux session not found", message);
        }

        // Count inbox/outbox
        let inbox_count = count_jsonl_lines(&spec.inbox).await;
        let outbox_count = count_jsonl_lines(&spec.outbox).await;

        Ok(WorkerHealth {
            worker_id: spec.name.clone(),
            status,
            last_heartbeat,
            heartbeat_content,
            tmux_pane_alive,
            inbox_count,
            outbox_count,
            message,
        })
    }

    /// Attempt to recover a dead worker by reloading its bridge script.
    pub async fn attempt_recovery(
        &self,
        worker_health: &WorkerHealth,
        state_dir: &Path,
        event_writer: &EventWriter,
        run_id: &RunId,
    ) -> Result<RecoveryResult> {
        if !self.config.attempt_recovery {
            return Ok(RecoveryResult {
                worker_id: worker_health.worker_id.clone(),
                action: "skipped".to_string(),
                success: false,
                message: "attempt_recovery is disabled in config".to_string(),
            });
        }

        if worker_health.status != HealthStatus::Dead {
            return Ok(RecoveryResult {
                worker_id: worker_health.worker_id.clone(),
                action: "skipped".to_string(),
                success: false,
                message: format!("Worker status is {:?}, not Dead", worker_health.status),
            });
        }

        let workers_dir = state_dir.join(WORKERS_DIR);
        let worker_dir = workers_dir.join(&worker_health.worker_id);
        let spec = match WorkerSpec::load(&worker_dir).await {
            Ok(s) => s,
            Err(e) => {
                return Ok(RecoveryResult {
                    worker_id: worker_health.worker_id.clone(),
                    action: "load_spec".to_string(),
                    success: false,
                    message: format!("Failed to load worker spec: {}", e),
                });
            }
        };

        // Derive pane index from worker name (worker-N -> pane N+1).
        let pane_index = if let Some(n_str) = spec.name.strip_prefix("worker-") {
            match n_str.parse::<usize>() {
                Ok(n) => n + 1,
                Err(_) => {
                    return Ok(RecoveryResult {
                        worker_id: worker_health.worker_id.clone(),
                        action: "parse_pane_index".to_string(),
                        success: false,
                        message: format!("Cannot parse pane index from worker name: {}", spec.name),
                    });
                }
            }
        } else {
            return Ok(RecoveryResult {
                worker_id: worker_health.worker_id.clone(),
                action: "parse_pane_index".to_string(),
                success: false,
                message: format!(
                    "Worker name does not follow worker-N pattern: {}",
                    spec.name
                ),
            });
        };

        let team_name = &run_id.0;
        let session_name = format!("omk-team-{team_name}");
        let window_name = "lead";

        match crate::runtime::tmux::pane_exists(&session_name, window_name, pane_index) {
            Ok(true) => {
                // Pane exists — respawn the bridge script.
                let bridge = TeamBridge::new(&spec, &session_name);
                match bridge.spawn_worker(pane_index).await {
                    Ok(()) => {
                        let event = Event::new(run_id.clone(), EventKind::WorkerRecovered)
                            .with_actor(&spec.name)
                            .with_message(format!("Respawned bridge in pane {}", pane_index))
                            .unwrap_or_else(|_| {
                                Event::new(run_id.clone(), EventKind::WorkerRecovered)
                                    .with_actor(&spec.name)
                            });
                        let _ = event_writer.append(&event).await;

                        Ok(RecoveryResult {
                            worker_id: worker_health.worker_id.clone(),
                            action: "respawn_bridge".to_string(),
                            success: true,
                            message: format!(
                                "Respawned bridge for {} in pane {}",
                                spec.name, pane_index
                            ),
                        })
                    }
                    Err(e) => Ok(RecoveryResult {
                        worker_id: worker_health.worker_id.clone(),
                        action: "respawn_bridge".to_string(),
                        success: false,
                        message: format!("Failed to respawn bridge: {}", e),
                    }),
                }
            }
            Ok(false) => Ok(RecoveryResult {
                worker_id: worker_health.worker_id.clone(),
                action: "check_pane".to_string(),
                success: false,
                message: format!("Tmux pane {} does not exist", pane_index),
            }),
            Err(e) => Ok(RecoveryResult {
                worker_id: worker_health.worker_id.clone(),
                action: "check_pane".to_string(),
                success: false,
                message: format!("Failed to check tmux pane: {}", e),
            }),
        }
    }
}

async fn count_jsonl_lines(path: &Path) -> usize {
    if !path.exists() {
        return 0;
    }
    match tokio::fs::read_to_string(path).await {
        Ok(content) => content.lines().filter(|l| !l.trim().is_empty()).count(),
        Err(_) => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn watchdog_healthy_worker() {
        let tmp = tempfile::tempdir().unwrap();
        let spec = WorkerSpec {
            name: "w1".to_string(),
            role: "coder".to_string(),
            inbox: tmp.path().join("inbox.jsonl"),
            outbox: tmp.path().join("outbox.jsonl"),
            heartbeat: tmp.path().join("heartbeat.json"),
            project_dir: None,
        };

        // Write a fresh heartbeat
        let hb = serde_json::json!({
            "status": "alive",
            "ts": Utc::now().to_rfc3339(),
        });
        tokio::fs::write(&spec.heartbeat, hb.to_string())
            .await
            .unwrap();

        let wd = Watchdog::with_defaults();
        let health = wd.check_worker(&spec, true).await.unwrap();
        assert_eq!(health.status, HealthStatus::Healthy);
    }

    #[tokio::test]
    async fn watchdog_missing_heartbeat() {
        let tmp = tempfile::tempdir().unwrap();
        let spec = WorkerSpec {
            name: "w1".to_string(),
            role: "coder".to_string(),
            inbox: tmp.path().join("inbox.jsonl"),
            outbox: tmp.path().join("outbox.jsonl"),
            heartbeat: tmp.path().join("heartbeat.json"),
            project_dir: None,
        };

        // Heartbeat file does not exist
        let wd = Watchdog::with_defaults();
        let health = wd.check_worker(&spec, true).await.unwrap();
        assert_eq!(health.status, HealthStatus::Dead);
    }

    #[tokio::test]
    async fn watchdog_stale_heartbeat() {
        let tmp = tempfile::tempdir().unwrap();
        let spec = WorkerSpec {
            name: "w1".to_string(),
            role: "coder".to_string(),
            inbox: tmp.path().join("inbox.jsonl"),
            outbox: tmp.path().join("outbox.jsonl"),
            heartbeat: tmp.path().join("heartbeat.json"),
            project_dir: None,
        };

        // Write an old heartbeat
        let old_ts = Utc::now() - chrono::Duration::seconds(120);
        let hb = serde_json::json!({
            "status": "alive",
            "ts": old_ts.to_rfc3339(),
        });
        tokio::fs::write(&spec.heartbeat, hb.to_string())
            .await
            .unwrap();

        let wd = Watchdog::new(WatchdogConfig {
            heartbeat_missing_secs: 300,
            heartbeat_stale_secs: 60,
            ..Default::default()
        });
        let health = wd.check_worker(&spec, true).await.unwrap();
        assert_eq!(health.status, HealthStatus::Stalled);
    }

    #[tokio::test]
    async fn watchdog_recovery_skipped_when_disabled() {
        let tmp = tempfile::tempdir().unwrap();
        let spec = WorkerSpec {
            name: "worker-0".to_string(),
            role: "coder".to_string(),
            inbox: tmp.path().join("inbox.jsonl"),
            outbox: tmp.path().join("outbox.jsonl"),
            heartbeat: tmp.path().join("heartbeat.json"),
            project_dir: None,
        };
        spec.save().await.unwrap();

        let wd = Watchdog::with_defaults();
        let health = wd.check_worker(&spec, true).await.unwrap();
        assert_eq!(health.status, HealthStatus::Dead);

        let event_log = tmp.path().join("events.jsonl");
        let event_writer = EventWriter::new(&event_log);
        let run_id = RunId("test-team".to_string());

        let result = wd
            .attempt_recovery(&health, tmp.path(), &event_writer, &run_id)
            .await
            .unwrap();

        assert!(!result.success);
        assert_eq!(result.action, "skipped");
        assert!(result.message.contains("disabled"));
    }

    #[tokio::test]
    async fn watchdog_recovery_skipped_for_healthy_worker() {
        let tmp = tempfile::tempdir().unwrap();
        let spec = WorkerSpec {
            name: "worker-0".to_string(),
            role: "coder".to_string(),
            inbox: tmp.path().join("inbox.jsonl"),
            outbox: tmp.path().join("outbox.jsonl"),
            heartbeat: tmp.path().join("heartbeat.json"),
            project_dir: None,
        };

        let hb = serde_json::json!({
            "status": "alive",
            "ts": Utc::now().to_rfc3339(),
        });
        tokio::fs::write(&spec.heartbeat, hb.to_string())
            .await
            .unwrap();

        let wd = Watchdog::new(WatchdogConfig {
            attempt_recovery: true,
            ..Default::default()
        });
        let health = wd.check_worker(&spec, true).await.unwrap();
        assert_eq!(health.status, HealthStatus::Healthy);

        let event_log = tmp.path().join("events.jsonl");
        let event_writer = EventWriter::new(&event_log);
        let run_id = RunId("test-team".to_string());

        let result = wd
            .attempt_recovery(&health, tmp.path(), &event_writer, &run_id)
            .await
            .unwrap();

        assert!(!result.success);
        assert_eq!(result.action, "skipped");
        assert!(result.message.contains("not Dead"));
    }

    #[tokio::test]
    async fn watchdog_healthy_worker_without_tmux_when_not_required() {
        let tmp = tempfile::tempdir().unwrap();
        let spec = WorkerSpec {
            name: "w1".to_string(),
            role: "coder".to_string(),
            inbox: tmp.path().join("inbox.jsonl"),
            outbox: tmp.path().join("outbox.jsonl"),
            heartbeat: tmp.path().join("heartbeat.json"),
            project_dir: None,
        };

        // Write a fresh heartbeat
        let hb = serde_json::json!({
            "status": "alive",
            "ts": Utc::now().to_rfc3339(),
        });
        tokio::fs::write(&spec.heartbeat, hb.to_string())
            .await
            .unwrap();

        let wd = Watchdog::new(WatchdogConfig {
            require_tmux: false,
            ..Default::default()
        });
        let health = wd.check_worker(&spec, false).await.unwrap();
        assert_eq!(health.status, HealthStatus::Healthy);
        assert!(!health.tmux_pane_alive);
    }
}
