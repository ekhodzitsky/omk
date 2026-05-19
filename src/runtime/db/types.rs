/// A persisted goal record.
#[derive(Debug, Clone)]
pub struct GoalRecord {
    pub goal_id: String,
    pub status: String,
    pub phase: String,
    pub kind: Option<String>,
    pub goal_text: String,
    pub project_dir: String,
    pub policy: String,
    pub merge_policy: String,
    pub slice_execution: bool,
    pub max_agents: Option<i32>,
    pub budget_time_secs: Option<i64>,
    pub budget_tokens: Option<i64>,
    pub budget_usd: Option<f64>,
    pub created_at: i64,
    pub updated_at: i64,
    pub controller_pid: Option<i32>,
    pub version: i32,
}

/// A persisted task record.
#[derive(Debug, Clone)]
pub struct TaskRecord {
    pub task_id: String,
    pub goal_id: String,
    pub kind: String,
    pub status: String,
    pub owner: Option<String>,
    pub write_set: Option<String>,
    pub depends_on: Option<String>,
    pub retry_count: i32,
    pub max_retries: i32,
    pub lease_expires_at: Option<i64>,
    pub evidence_paths: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

/// A persisted event record.
#[derive(Debug, Clone)]
pub struct EventRecord {
    pub event_id: i64,
    pub goal_id: String,
    pub kind: String,
    pub payload: String,
    pub created_at: i64,
}

/// A persisted proof record.
#[derive(Debug, Clone)]
pub struct ProofRecord {
    pub goal_id: String,
    pub status: String,
    pub gates_passed: i32,
    pub gates_total: i32,
    pub changed_files: Option<String>,
    pub known_gaps: Option<String>,
    pub recovery_status: Option<String>,
    pub generated_at: i64,
}

/// A budget checkpoint.
#[derive(Debug, Clone)]
pub struct BudgetCheckpoint {
    pub checkpoint_id: Option<i64>,
    pub goal_id: String,
    pub kind: String,
    pub limit_value: Option<f64>,
    pub used_value: Option<f64>,
    pub created_at: i64,
}

/// A registered artifact.
#[derive(Debug, Clone)]
pub struct ArtifactRecord {
    pub artifact_id: i64,
    pub goal_id: String,
    pub kind: String,
    pub path: String,
    pub mime_type: Option<String>,
    pub created_at: i64,
}

/// Filter for listing goals.
#[derive(Debug, Clone, Default)]
pub struct GoalFilter {
    pub status: Option<String>,
    pub phase: Option<String>,
    pub kind: Option<String>,
    pub older_than: Option<i64>,
    pub limit: Option<usize>,
}

/// Summary of a goal for list views.
#[derive(Debug, Clone)]
pub struct GoalSummary {
    pub goal_id: String,
    pub status: String,
    pub phase: String,
    pub goal_text: String,
    pub created_at: i64,
    pub updated_at: i64,
}
