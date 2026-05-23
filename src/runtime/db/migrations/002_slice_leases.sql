CREATE TABLE IF NOT EXISTS goal_slice_leases (
    lease_id TEXT PRIMARY KEY,
    goal_id TEXT NOT NULL,
    slice_id TEXT NOT NULL,
    owner_pid INTEGER NOT NULL,
    owner_role TEXT NOT NULL,
    claimed_at INTEGER NOT NULL,
    heartbeat_at INTEGER NOT NULL,
    released_at INTEGER,
    expired_at INTEGER,
    write_set TEXT NOT NULL,
    UNIQUE (goal_id, slice_id, released_at, expired_at),
    FOREIGN KEY (goal_id) REFERENCES goals(goal_id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_slice_leases_goal
    ON goal_slice_leases(goal_id);

CREATE INDEX IF NOT EXISTS idx_slice_leases_active
    ON goal_slice_leases(goal_id, slice_id)
    WHERE released_at IS NULL AND expired_at IS NULL;
