use chrono::{DateTime, Utc};

/// Summary data for a completed session, produced by runtime modules and
/// consumed by the CLI layer for cost tracking and notifications.
///
/// This type intentionally carries no `cost` or `notification` dependencies so
/// that `runtime/` can remain decoupled from `cost/` and `notifications/`.
#[derive(Debug, Clone)]
pub struct SessionSummary {
    pub session_type: String,
    pub name: String,
    pub started_at: DateTime<Utc>,
    pub ended_at: DateTime<Utc>,
    pub duration_secs: u64,
    pub jobs_total: Option<usize>,
    pub jobs_success: Option<usize>,
    pub phases_completed: Option<usize>,
    pub iterations: Option<usize>,
    pub verified: Option<usize>,
    pub total_stories: Option<usize>,
}
