PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;
PRAGMA synchronous = NORMAL;

CREATE TABLE IF NOT EXISTS goals (
    goal_id TEXT PRIMARY KEY,
    status TEXT NOT NULL,
    phase TEXT NOT NULL,
    kind TEXT,
    original_goal TEXT NOT NULL DEFAULT '',
    normalized_goal TEXT NOT NULL DEFAULT '',
    goal_text TEXT NOT NULL,
    project_dir TEXT NOT NULL,
    state_dir TEXT NOT NULL DEFAULT '',
    policy TEXT NOT NULL DEFAULT 'local',
    delivery_policy TEXT NOT NULL DEFAULT 'local',
    merge_policy TEXT NOT NULL DEFAULT 'disabled',
    until_ready INTEGER NOT NULL DEFAULT 0 CHECK(until_ready IN (0, 1)),
    slice_execution INTEGER NOT NULL DEFAULT 0 CHECK(slice_execution IN (0, 1)),
    max_agents INTEGER,
    budget_time_secs INTEGER,
    budget_tokens INTEGER,
    budget_usd INTEGER,
    cost_tracker_path TEXT,
    terminal_criteria TEXT,
    failure TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    completed_at INTEGER,
    controller_pid INTEGER,
    version INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE IF NOT EXISTS tasks (
    task_id TEXT PRIMARY KEY,
    goal_id TEXT NOT NULL REFERENCES goals(goal_id) ON DELETE CASCADE,
    title TEXT NOT NULL DEFAULT '',
    description TEXT NOT NULL DEFAULT '',
    kind TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    owner TEXT,
    read_set TEXT,
    write_set TEXT,
    depends_on TEXT,
    risk TEXT NOT NULL DEFAULT '',
    acceptance TEXT,
    evidence TEXT,
    retry_count INTEGER DEFAULT 0,
    max_retries INTEGER DEFAULT 3,
    lease_expires_at INTEGER,
    completed_at INTEGER,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_tasks_goal_id ON tasks(goal_id);

CREATE TABLE IF NOT EXISTS events (
    event_id INTEGER PRIMARY KEY AUTOINCREMENT,
    event_uuid TEXT,
    run_id TEXT,
    goal_id TEXT NOT NULL REFERENCES goals(goal_id) ON DELETE CASCADE,
    schema_version INTEGER NOT NULL DEFAULT 1,
    kind TEXT NOT NULL,
    actor TEXT,
    payload TEXT NOT NULL,
    created_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_events_goal_id_created ON events(goal_id, created_at);

CREATE TABLE IF NOT EXISTS proofs (
    goal_id TEXT PRIMARY KEY REFERENCES goals(goal_id) ON DELETE CASCADE,
    version INTEGER NOT NULL DEFAULT 1,
    status TEXT NOT NULL,
    readiness TEXT NOT NULL DEFAULT '',
    summary TEXT NOT NULL DEFAULT '',
    task_graph_summary TEXT,
    changed_files TEXT,
    commits TEXT,
    git TEXT,
    gates TEXT,
    gates_passed INTEGER DEFAULT 0,
    gates_total INTEGER DEFAULT 0,
    post_mutation_gates_ran INTEGER NOT NULL DEFAULT 0,
    known_gaps TEXT,
    human_decisions_required TEXT,
    recovery_status TEXT,
    generated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS budget_checkpoints (
    checkpoint_id INTEGER PRIMARY KEY AUTOINCREMENT,
    goal_id TEXT NOT NULL REFERENCES goals(goal_id) ON DELETE CASCADE,
    version INTEGER NOT NULL DEFAULT 1,
    label TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT '',
    phase TEXT NOT NULL DEFAULT '',
    recorded_at INTEGER NOT NULL,
    budget_time TEXT,
    total_budget_secs INTEGER,
    elapsed_since_created_secs INTEGER NOT NULL DEFAULT 0,
    remaining_budget_secs INTEGER,
    budget_tokens INTEGER,
    used_tokens INTEGER NOT NULL DEFAULT 0,
    remaining_budget_tokens INTEGER,
    budget_usd INTEGER,
    estimated_cost_usd INTEGER NOT NULL DEFAULT 0,
    remaining_budget_usd INTEGER,
    limit_value INTEGER,
    used_value INTEGER,
    created_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_budget_checkpoints_goal_id ON budget_checkpoints(goal_id);

CREATE TABLE IF NOT EXISTS artifacts (
    artifact_id INTEGER PRIMARY KEY AUTOINCREMENT,
    goal_id TEXT NOT NULL REFERENCES goals(goal_id) ON DELETE CASCADE,
    kind TEXT NOT NULL,
    path TEXT NOT NULL,
    mime_type TEXT,
    created_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_artifacts_goal_id ON artifacts(goal_id);
