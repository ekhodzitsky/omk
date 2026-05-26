CREATE TABLE IF NOT EXISTS circuit_breakers (
    id TEXT PRIMARY KEY,
    gate_name TEXT NOT NULL,
    project_path TEXT NOT NULL,
    state TEXT NOT NULL,
    consecutive_failures INTEGER NOT NULL DEFAULT 0,
    failure_threshold INTEGER NOT NULL DEFAULT 5,
    recovery_timeout_secs INTEGER NOT NULL DEFAULT 30,
    half_open_max_calls INTEGER NOT NULL DEFAULT 1,
    half_open_calls_remaining INTEGER NOT NULL DEFAULT 0,
    last_failure_at TEXT,
    last_success_at TEXT,
    opened_at TEXT,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_circuit_breakers_project_path
    ON circuit_breakers(project_path);
