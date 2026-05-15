use anyhow::Result;

use crate::runtime::events::EventKind;
use crate::runtime::watchdog::HealthStatus;
use crate::vis::hud::sanitize::strip_ansi;
use crate::vis::hud::types::{HudState, WorkerDisplay};

impl HudState {
    /// Build per-worker display records combining health, events, and state.
    pub fn worker_displays(&self) -> Vec<WorkerDisplay> {
        self.workers
            .iter()
            .map(|w| {
                let current_task = self.find_worker_task(&w.worker_id);
                let retry_count = current_task
                    .as_ref()
                    .map(|t| self.count_retries_for_task(t))
                    .unwrap_or(0);

                let status = match w.status {
                    HealthStatus::Healthy => {
                        if current_task.is_some() {
                            "Busy"
                        } else {
                            "Ready"
                        }
                    }
                    HealthStatus::Stalled => "Stalled",
                    HealthStatus::Dead => "Dead",
                    HealthStatus::Unknown => "Unknown",
                }
                .to_string();

                let heartbeat_age_secs = if let Some(hb) = w.last_heartbeat {
                    self.last_update.signed_duration_since(hb).num_seconds()
                } else {
                    self.events
                        .iter()
                        .rev()
                        .find(|e| {
                            e.kind == EventKind::WorkerHeartbeat
                                && e.actor.as_deref() == Some(&w.worker_id)
                        })
                        .map(|e| self.last_update.signed_duration_since(e.ts).num_seconds())
                        .unwrap_or(-1)
                };

                WorkerDisplay {
                    name: w.worker_id.clone(),
                    status,
                    heartbeat_age_secs,
                    current_task_id: current_task,
                    retry_count,
                    gate_status: self.latest_gate_status(),
                }
            })
            .collect()
    }

    fn find_worker_task(&self, worker_id: &str) -> Option<String> {
        let mut task_id = None;
        for event in &self.events {
            if event.actor.as_deref() != Some(worker_id) {
                continue;
            }
            if let Some(ref payload) = event.payload {
                if let Some(tid) = payload.get("task_id").and_then(|v| v.as_str()) {
                    match event.kind {
                        EventKind::TaskClaimed | EventKind::TaskStarted => {
                            task_id = Some(tid.to_string());
                        }
                        EventKind::TaskCompleted | EventKind::TaskFailed => {
                            task_id = None;
                        }
                        _ => {}
                    }
                }
            }
        }
        task_id
    }

    fn count_retries_for_task(&self, task_id: &str) -> usize {
        self.events
            .iter()
            .filter(|e| {
                e.kind == EventKind::RetryScheduled
                    && e.payload
                        .as_ref()
                        .and_then(|p| p.get("task_id").and_then(|v| v.as_str()))
                        == Some(task_id)
            })
            .count()
    }

    fn latest_gate_status(&self) -> String {
        for event in self.events.iter().rev() {
            match event.kind {
                EventKind::GatePassed => return "passed".to_string(),
                EventKind::GateFailed => return "failed".to_string(),
                _ => {}
            }
        }
        "-".to_string()
    }

    /// Render as formatted text for terminal output.
    pub fn render_text(&self) -> String {
        let runtime = self.last_update.signed_duration_since(self.start_time);
        let runtime_str = format!(
            "{}:{:02}:{:02}",
            runtime.num_hours(),
            runtime.num_minutes().rem_euclid(60),
            runtime.num_seconds().rem_euclid(60)
        );

        let healthy_count = self
            .workers
            .iter()
            .filter(|w| w.status == HealthStatus::Healthy)
            .count();
        let stalled_count = self
            .workers
            .iter()
            .filter(|w| w.status == HealthStatus::Stalled)
            .count();
        let dead_count = self
            .workers
            .iter()
            .filter(|w| w.status == HealthStatus::Dead)
            .count();

        let mut lines = Vec::new();
        lines.push(format!(
            "OMK HUD — team: {} | run: {}",
            strip_ansi(&self.team_name),
            strip_ansi(&self.run_id)
        ));
        lines.push("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".to_string());
        lines.push(format!(
            "Workers: {} total | {} healthy | {} stalled | {} dead",
            self.workers.len(),
            healthy_count,
            stalled_count,
            dead_count
        ));
        lines.push(format!(
            "Tasks:   {} total | {} completed | {} running | {} pending",
            self.task_summary.total,
            self.task_summary.completed,
            self.task_summary.running,
            self.task_summary.pending
        ));
        lines.push(format!("Runtime: {}", runtime_str));
        lines.push(format!("Events:  {}", self.events.len()));
        if let Some(gate_name) = self.latest_failed_gate() {
            lines.push(format!("Gate:    {} (failed)", strip_ansi(&gate_name)));
        }
        if let Some(status) = self.latest_proof_status() {
            lines.push(format!("Proof:   {}", strip_ansi(&status)));
        }
        lines.push("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".to_string());

        for display in self.worker_displays() {
            let (icon, _) = match display.status.as_str() {
                "Healthy" | "Ready" | "Busy" => ("✅", "healthy"),
                "Stalled" => ("⚠️", "stalled"),
                "Dead" => ("❌", "dead"),
                _ => ("❓", "unknown"),
            };

            let age_str = if display.heartbeat_age_secs >= 0 {
                format!("{}s", display.heartbeat_age_secs)
            } else {
                "N/A".to_string()
            };

            let task_str = display
                .current_task_id
                .map(|t| strip_ansi(&t))
                .unwrap_or_else(|| "-".to_string());

            lines.push(format!(
                "[{}] {} {} | age: {} | task: {} | retries: {} | gates: {}",
                strip_ansi(&display.name),
                icon,
                display.status,
                age_str,
                task_str,
                display.retry_count,
                strip_ansi(&display.gate_status),
            ));
        }

        lines.join("\n")
    }

    /// Render as JSON (for API consumption)
    pub fn render_json(&self) -> Result<String> {
        let json = serde_json::to_string_pretty(self)?;
        Ok(json)
    }

    fn latest_failed_gate(&self) -> Option<String> {
        if let Some(name) = &self.latest_failed_gate {
            return Some(name.clone());
        }
        Self::extract_latest_failed_gate(&self.events)
    }

    fn latest_proof_status(&self) -> Option<String> {
        if let Some(status) = &self.proof_status {
            return Some(status.clone());
        }
        Self::extract_latest_proof_status(&self.events)
    }
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, Utc};

    use crate::runtime::events::{Event, EventKind, RunId};
    use crate::runtime::watchdog::{HealthStatus, WorkerHealth};
    use crate::vis::hud::types::{HudState, TaskSummary};

    #[test]
    fn hud_state_render_text_expected_output() {
        let mut hud = HudState::new("my-team", "abc123");
        hud.start_time = Utc::now() - Duration::seconds(154);
        hud.last_update = Utc::now();
        hud.events.push(Event::new(
            RunId("abc123".to_string()),
            EventKind::RunStarted,
        ));
        hud.events.push(
            Event::new(RunId("abc123".to_string()), EventKind::WorkerStarted)
                .with_actor("worker-0"),
        );
        hud.workers.push(WorkerHealth {
            worker_id: "worker-0".to_string(),
            status: HealthStatus::Healthy,
            last_heartbeat: Some(Utc::now()),
            heartbeat_content: None,
            inbox_count: 0,
            outbox_count: 0,
            message: "Heartbeat fresh (5s ago)".to_string(),
        });
        hud.task_summary = TaskSummary {
            total: 5,
            completed: 2,
            running: 1,
            pending: 2,
            failed: 0,
        };

        let text = hud.render_text();
        assert!(text.contains("team: my-team"));
        assert!(text.contains("run: abc123"));
        assert!(text.contains("Workers: 1 total | 1 healthy | 0 stalled | 0 dead"));
        assert!(text.contains("Tasks:   5 total | 2 completed | 1 running | 2 pending"));
        assert!(text.contains("Runtime: 0:02:34"));
        assert!(text.contains("Events:  2"));
        assert!(text.contains("[worker-0] ✅ Ready"));
    }
}
