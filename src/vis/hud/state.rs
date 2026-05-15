use std::collections::HashSet;
use std::path::Path;

use anyhow::Result;
use chrono::Utc;

use crate::runtime::events::{Event, EventKind, EventReader, RunId};
use crate::runtime::state::{TaskStatus, TeamState};
use crate::runtime::watchdog::Watchdog;
use crate::vis::event_stream::EventStream;
use crate::vis::hud::types::{HudState, TaskSummary};

impl HudState {
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

    pub(crate) fn extract_latest_failed_gate(events: &[Event]) -> Option<String> {
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

    pub(crate) fn extract_latest_proof_status(events: &[Event]) -> Option<String> {
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
