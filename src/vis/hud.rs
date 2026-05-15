use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::collections::HashSet;
use std::path::Path;

use crate::runtime::events::{Event, EventKind, EventReader, RunId};
use crate::runtime::state::{TaskStatus, TeamState};
use crate::runtime::watchdog::{HealthStatus, Watchdog, WorkerHealth};
use crate::vis::event_stream::EventStream;

/// Strip ANSI escape sequences and unsafe control bytes from worker- or
/// event-log-supplied text before rendering it to a terminal.
///
/// Without this, a worker that writes `\x1B[2J\x1B[H` (clear screen + home)
/// or OSC sequences into its heartbeat / event payload can take over the
/// HUD's terminal. Tab and newline are preserved so multi-line evidence
/// renders normally.
pub(crate) fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1B' {
            match chars.next() {
                Some('[') => {
                    // CSI: 0x40..=0x7E terminates.
                    while let Some(&p) = chars.peek() {
                        chars.next();
                        if matches!(p, '\x40'..='\x7E') {
                            break;
                        }
                    }
                }
                Some(']') | Some('P') | Some('X') | Some('^') | Some('_') => {
                    // String-bracketed sequences: OSC (`]`), DCS (`P`), SOS
                    // (`X`), PM (`^`), APC (`_`). All are terminated by BEL
                    // (0x07) or ESC `\`. Treat their bodies as opaque so an
                    // attacker cannot smuggle visible payload through.
                    while let Some(&p) = chars.peek() {
                        chars.next();
                        if p == '\x07' {
                            break;
                        }
                        if p == '\x1B' {
                            if chars.peek() == Some(&'\\') {
                                chars.next();
                            }
                            break;
                        }
                    }
                }
                Some(_) => {
                    // Two-byte ESC sequence (ESC X) — already consumed second
                    // byte; nothing more to skip.
                }
                None => break,
            }
        } else if c == '\t' || c == '\n' || !c.is_control() {
            out.push(c);
        }
    }
    out
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct TaskSummary {
    pub total: usize,
    pub completed: usize,
    pub running: usize,
    pub pending: usize,
    pub failed: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkerDisplay {
    pub name: String,
    pub status: String,
    pub heartbeat_age_secs: i64,
    pub current_task_id: Option<String>,
    pub retry_count: usize,
    pub gate_status: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct HudState {
    pub run_id: String,
    pub team_name: String,
    pub events: Vec<Event>,
    pub workers: Vec<WorkerHealth>,
    pub task_summary: TaskSummary,
    pub start_time: DateTime<Utc>,
    pub last_update: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proof_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_failed_gate: Option<String>,
    #[serde(skip)]
    pub team_state: Option<TeamState>,
}

impl HudState {
    pub fn new(team_name: &str, run_id: &str) -> Self {
        Self {
            run_id: run_id.to_string(),
            team_name: team_name.to_string(),
            events: Vec::new(),
            workers: Vec::new(),
            task_summary: TaskSummary::default(),
            start_time: Utc::now(),
            last_update: Utc::now(),
            proof_status: None,
            latest_failed_gate: None,
            team_state: None,
        }
    }

    /// Update state from event stream and health check
    pub async fn refresh(
        &mut self,
        event_stream: &mut EventStream,
        watchdog: &Watchdog,
        state_dir: &Path,
    ) -> Result<()> {
        let new_events = event_stream.poll().await?;
        self.events.extend(new_events);

        // L5-001: Also read the full events.jsonl to ensure completeness
        let events_path = state_dir.join("events.jsonl");
        if let Ok(all_events) = EventReader::read_all(&events_path).await {
            let existing_ids: HashSet<_> = self.events.iter().map(|e| e.id.clone()).collect();
            for event in all_events {
                if !existing_ids.contains(&event.id) {
                    self.events.push(event);
                }
            }
            self.events.sort_by_key(|a| a.ts);
        }

        // Determine start time from RunStarted event or fall back to team state
        if let Some(start_event) = self
            .events
            .iter()
            .find(|e| matches!(e.kind, EventKind::RunStarted))
        {
            self.start_time = start_event.ts;
        } else if let Ok(team_state) = TeamState::load(state_dir).await {
            self.start_time = team_state.created_at;
        }

        self.latest_failed_gate = Self::extract_latest_failed_gate(&self.events);
        self.proof_status = Self::extract_latest_proof_status(&self.events);

        // Load team state for task summary and store it
        self.team_state = TeamState::load(state_dir).await.ok();
        let mut summary = TaskSummary::default();

        if let Some(ref state) = self.team_state {
            summary.total = state.tasks.len();
            for task in &state.tasks {
                match task.status {
                    TaskStatus::Done => summary.completed += 1,
                    TaskStatus::InProgress => summary.running += 1,
                    TaskStatus::Pending => summary.pending += 1,
                    TaskStatus::Failed => summary.failed += 1,
                }
            }
        }

        // If no tasks in state, try events as fallback
        if summary.total == 0 {
            let mut running_set = HashSet::new();
            let mut completed_set = HashSet::new();
            let mut failed_set = HashSet::new();

            for event in &self.events {
                if let Some(ref payload) = event.payload {
                    if let Some(tid) = payload.get("task_id").and_then(|v| v.as_str()) {
                        match event.kind {
                            EventKind::TaskClaimed | EventKind::TaskStarted
                                if !completed_set.contains(tid) && !failed_set.contains(tid) =>
                            {
                                running_set.insert(tid.to_string());
                            }
                            EventKind::TaskCompleted => {
                                running_set.remove(tid);
                                completed_set.insert(tid.to_string());
                            }
                            EventKind::TaskFailed => {
                                running_set.remove(tid);
                                failed_set.insert(tid.to_string());
                            }
                            _ => {}
                        }
                    }
                }
            }

            summary.running = running_set.len();
            summary.completed = completed_set.len();
            summary.failed = failed_set.len();
            summary.total = summary.running + summary.completed + summary.failed;
        }

        self.task_summary = summary;

        // Run health check (read-only)
        let run_id = RunId(self.run_id.clone());
        let report = watchdog.check_team_read_only(&run_id, state_dir).await?;
        self.workers = report.workers;

        self.last_update = Utc::now();
        Ok(())
    }

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

    fn extract_latest_failed_gate(events: &[Event]) -> Option<String> {
        for event in events.iter().rev() {
            if event.kind == EventKind::GateFailed {
                if let Some(ref payload) = event.payload {
                    if let Some(name) = payload.get("name").and_then(|v| v.as_str()) {
                        return Some(name.to_string());
                    }
                }
            }
        }
        None
    }

    fn latest_proof_status(&self) -> Option<String> {
        if let Some(status) = &self.proof_status {
            return Some(status.clone());
        }
        Self::extract_latest_proof_status(&self.events)
    }

    fn extract_latest_proof_status(events: &[Event]) -> Option<String> {
        for event in events.iter().rev() {
            if event.kind == EventKind::ProofWritten {
                if let Some(ref payload) = event.payload {
                    if let Some(status) = payload.get("status").and_then(|v| v.as_str()) {
                        return Some(status.to_string());
                    }
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::events::{Event, EventKind, RunId};

    #[test]
    fn strip_ansi_removes_csi_clear_screen() {
        // \x1B[2J = ED (Erase in Display), \x1B[H = CUP (Cursor Position).
        // A worker that wrote these into its heartbeat must not be able to
        // wipe the HUD's terminal.
        assert_eq!(strip_ansi("\x1B[2J\x1B[Hhello"), "hello");
    }

    #[test]
    fn strip_ansi_removes_osc_bel_terminated() {
        assert_eq!(strip_ansi("\x1B]0;evil-title\x07ok"), "ok");
    }

    #[test]
    fn strip_ansi_removes_osc_st_terminated() {
        // OSC terminated by ESC \\.
        assert_eq!(strip_ansi("\x1B]0;evil-title\x1B\\ok"), "ok");
    }

    #[test]
    fn strip_ansi_removes_dcs_family() {
        // DCS (ESC P), APC (ESC _), PM (ESC ^), SOS (ESC X). Each must
        // consume its opaque payload, not leak it into the output.
        assert_eq!(strip_ansi("\x1BPpayload\x1B\\after"), "after");
        assert_eq!(strip_ansi("\x1B_apc-data\x07after"), "after");
        assert_eq!(strip_ansi("\x1B^pm-data\x07after"), "after");
        assert_eq!(strip_ansi("\x1BXsos-data\x1B\\after"), "after");
    }

    #[test]
    fn strip_ansi_preserves_tab_and_newline() {
        assert_eq!(strip_ansi("a\tb\nc"), "a\tb\nc");
    }

    #[test]
    fn strip_ansi_strips_bare_control_bytes() {
        assert_eq!(strip_ansi("bell\x07ok"), "bellok");
        assert_eq!(strip_ansi("nul\0ok"), "nulok");
    }

    #[test]
    fn strip_ansi_two_byte_esc_sequence() {
        // ESC M (RI - Reverse Index). Two-byte sequence: drop both.
        assert_eq!(strip_ansi("a\x1BMb"), "ab");
    }

    #[test]
    fn strip_ansi_does_not_panic_on_unterminated_sequence() {
        // Pathological input: ESC at the very end.
        assert_eq!(strip_ansi("trailing\x1B"), "trailing");
        // CSI with no terminator → consumed to EOF, returns empty tail.
        assert_eq!(strip_ansi("a\x1B[999"), "a");
    }

    #[test]
    fn strip_ansi_passes_through_plain_text() {
        assert_eq!(strip_ansi("hello world"), "hello world");
        assert_eq!(strip_ansi("emoji ✅ ok"), "emoji ✅ ok");
    }

    #[test]
    fn hud_state_render_text_expected_output() {
        let mut hud = HudState::new("my-team", "abc123");
        hud.start_time = Utc::now() - chrono::Duration::seconds(154);
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
