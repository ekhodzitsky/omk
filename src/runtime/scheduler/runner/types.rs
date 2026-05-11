use serde::Deserialize;

/// Summary of a completed (or failed) team run.
#[derive(Debug, Clone)]
pub struct RunSummary {
    pub run_id: String,
    pub completed: usize,
    pub failed: usize,
    pub cancelled: usize,
    pub total: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct SimpleResult {
    pub id: String,
    pub status: String,
    #[serde(default)]
    pub result: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
}

pub(crate) struct ParsedResult {
    pub task_id: String,
    pub status: String,
    pub summary: String,
    pub error: String,
}
