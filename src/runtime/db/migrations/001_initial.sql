PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;
PRAGMA synchronous = NORMAL;

CREATE TABLE IF NOT EXISTS goals (
    goal_id TEXT PRIMARY KEY,
    status TEXT NOT NULL,
    phase TEXT NOT NULL,
    kind TEXT,
    goal_text TEXT NOT NULL,
    project_dir TEXT NOT NULL,
    policy TEXT NOT NULL DEFAULT 'local',
    merge_policy TEXT NOT NULL DEFAULT 'disabled',
    slice_execution INTEGER NOT NULL DEFAULT 0,
    max_agents INTEGER,
    budget_time_secs INTEGER,
    budget_tokens INTEGER,
    budget_usd REAL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    controller_pid INTEGER,
    version INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE IF NOT EXISTS tasks (
    task_id TEXT PRIMARY KEY,
    goal_id TEXT NOT NULL REFERENCES goals(goal_id) ON DELETE CASCADE,
    kind TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    owner TEXT,
    write_set TEXT,
    depends_on TEXT,
    retry_count INTEGER DEFAULT 0,
    max_retries INTEGER DEFAULT 3,
    lease_expires_at INTEGER,
    evidence_paths TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_tasks_goal_id ON tasks(goal_id);

CREATE TABLE IF NOT EXISTS events (
    event_id INTEGER PRIMARY KEY AUTOINCREMENT,
    goal_id TEXT NOT NULL REFERENCES goals(goal_id) ON DELETE CASCADE,
    kind TEXT NOT NULL,
    payload TEXT NOT NULL,
    created_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_events_goal_id_created ON events(goal_id, created_at);

CREATE TABLE IF NOT EXISTS proofs (
    goal_id TEXT PRIMARY KEY REFERENCES goals(goal_id) ON DELETE CASCADE,
    status TEXT NOT NULL,
    gates_passed INTEGER DEFAULT 0,
    gates_total INTEGER DEFAULT 0,
    changed_files TEXT,
    known_gaps TEXT,
    recovery_status TEXT,
    generated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS budget_checkpoints (
    checkpoint_id INTEGER PRIMARY KEY AUTOINCREMENT,
    goal_id TEXT NOT NULL REFERENCES goals(goal_id) ON DELETE CASCADE,
    kind TEXT NOT NULL,
    limit_value REAL,
    used_value REAL,
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS artifacts (
    artifact_id INTEGER PRIMARY KEY AUTOINCREMENT,
    goal_id TEXT NOT NULL REFERENCES goals(goal_id) ON DELETE CASCADE,
    kind TEXT NOT NULL,
    path TEXT NOT NULL,
    mime_type TEXT,
    created_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_artifacts_goal_id ON artifacts(goal_id);
