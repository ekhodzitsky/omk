use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::runtime::gates::GateResult;

/// Full autopilot state persisted as JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutopilotState {
    #[serde(default = "crate::runtime::state::default_state_version")]
    pub version: u32,
    pub task: String,
    pub phase: AutopilotPhase,
    pub plans_dir: PathBuf,
    pub created_at: DateTime<Utc>,
    pub current_plan: Option<String>,
    pub qa_results: Option<QaResults>,
    #[serde(default)]
    pub gate_results: Vec<GateResult>,
    pub validation_results: Vec<ValidationResult>,
    pub execution_log: Vec<PhaseLog>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseLog {
    pub phase: String,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub success: bool,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QaResults {
    pub passed: bool,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub reviewer: String,
    pub passed: bool,
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub enum AutopilotPhase {
    Expansion,
    Planning,
    Execution,
    Qa,
    Validation,
    Cleanup,
    Complete,
    Failed,
}
