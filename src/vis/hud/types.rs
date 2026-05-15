use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::runtime::events::Event;
use crate::runtime::state::TeamState;
use crate::runtime::watchdog::WorkerHealth;

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
}
