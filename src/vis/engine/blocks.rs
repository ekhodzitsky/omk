use std::path::PathBuf;

use chrono::{DateTime, Utc};

use crate::vis::bus::Intent;

#[derive(Debug, Default, Clone)]
pub struct SessionInfo {
    pub id: String,
    pub project_short: String,
    pub started_at: DateTime<Utc>,
    pub uptime: std::time::Duration,
}

#[derive(Debug, Clone)]
pub struct ClassifierBlock {
    pub intent: Intent,
    pub confidence: f32,
    pub latency_ms: u32,
    pub reasoning: String,
    pub ts: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct PlanBlock {
    pub goal_id: String,
    pub nodes: Vec<crate::vis::bus::PlanNode>,
    pub revision: u32,
}

#[derive(Debug, Clone)]
pub struct WorkerBlock {
    pub worker_id: String,
    pub kind: String,
    pub task: String,
    pub status: WorkerStatus,
    pub percent: Option<f32>,
    pub message: Option<String>,
    pub started_at: DateTime<Utc>,
}

impl WorkerBlock {
    pub fn elapsed(&self, now: DateTime<Utc>) -> std::time::Duration {
        (now - self.started_at)
            .to_std()
            .unwrap_or(std::time::Duration::ZERO)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerStatus {
    Running,
    Done,
    Failed,
}

#[derive(Debug, Clone)]
pub struct GateBlock {
    pub gate: String,
    pub state: String,
}

#[derive(Debug, Clone)]
pub struct SliceBlock {
    pub slice_id: String,
    pub worktree: PathBuf,
    pub pr_url: Option<String>,
}

#[derive(Debug, Default, Clone)]
pub struct CostBlock {
    pub tokens_in: u64,
    pub tokens_out: u64,
    pub usd: f32,
}
