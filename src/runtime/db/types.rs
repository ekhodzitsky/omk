/// A persisted goal record.
#[derive(Debug, Clone)]
pub struct GoalRecord {
    pub goal_id: String,
    pub status: String,
    pub phase: String,
    pub kind: Option<String>,
    pub original_goal: String,
    pub normalized_goal: String,
    pub goal_text: String,
    pub project_dir: String,
    pub state_dir: String,
    pub policy: String,
    pub delivery_policy: String,
    pub merge_policy: String,
    pub until_ready: bool,
    pub slice_execution: bool,
    pub max_agents: Option<i32>,
    pub budget_time_secs: Option<i64>,
    pub budget_tokens: Option<i64>,
    pub budget_usd: Option<i64>, // cents
    pub cost_tracker_path: Option<String>,
    pub terminal_criteria: Option<String>, // JSON
    pub failure: Option<String>,          // JSON
    pub created_at: i64,
    pub updated_at: i64,
    pub completed_at: Option<i64>,
    pub controller_pid: Option<i32>,
    pub version: i32,
}

/// A persisted task record.
#[derive(Debug, Clone)]
pub struct TaskRecord {
    pub task_id: String,
    pub goal_id: String,
    pub title: String,
    pub description: String,
    pub kind: String,
    pub status: String,
    pub owner: Option<String>,
    pub read_set: Option<String>,      // JSON array
    pub write_set: Option<String>,     // JSON array
    pub depends_on: Option<String>,    // JSON array
    pub risk: String,
    pub acceptance: Option<String>,    // JSON array
    pub evidence: Option<String>,      // JSON array
    pub retry_count: i32,
    pub max_retries: i32,
    pub lease_expires_at: Option<i64>,
    pub completed_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

/// A persisted event record.
#[derive(Debug, Clone)]
pub struct EventRecord {
    pub event_id: i64,
    pub event_uuid: Option<String>,
    pub run_id: Option<String>,
    pub goal_id: String,
    pub schema_version: i32,
    pub kind: String,
    pub actor: Option<String>,
    pub payload: String, // JSON
    pub created_at: i64,
}

/// A persisted proof record.
#[derive(Debug, Clone)]
pub struct ProofRecord {
    pub goal_id: String,
    pub version: i32,
    pub status: String,
    pub readiness: String,
    pub summary: String,
    pub task_graph_summary: Option<String>, // JSON
    pub changed_files: Option<String>,      // JSON array
    pub commits: Option<String>,            // JSON array
    pub git: Option<String>,                // JSON
    pub gates: Option<String>,              // JSON array
    pub gates_passed: i32,
    pub gates_total: i32,
    pub post_mutation_gates_ran: bool,
    pub known_gaps: Option<String>,         // JSON array
    pub human_decisions_required: Option<String>, // JSON array
    pub recovery_status: Option<String>,
    pub generated_at: i64,
}

/// A budget checkpoint.
#[derive(Debug, Clone)]
pub struct BudgetCheckpoint {
    pub checkpoint_id: Option<i64>,
    pub goal_id: String,
    pub version: i32,
    pub label: String,
    pub status: String,
    pub phase: String,
    pub recorded_at: i64,
    pub budget_time: Option<String>,
    pub total_budget_secs: Option<i64>,
    pub elapsed_since_created_secs: i64,
    pub remaining_budget_secs: Option<i64>,
    pub budget_tokens: Option<i64>,
    pub used_tokens: i64,
    pub remaining_budget_tokens: Option<i64>,
    pub budget_usd: Option<i64>, // cents
    pub estimated_cost_usd: i64, // cents
    pub remaining_budget_usd: Option<i64>, // cents
    pub limit_value: Option<i64>,
    pub used_value: Option<i64>,
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
    pub older_than: Option<i64>, // unix seconds
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
