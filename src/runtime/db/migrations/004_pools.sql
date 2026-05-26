CREATE TABLE IF NOT EXISTS pool_queue (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    pool_name TEXT NOT NULL,
    task_id TEXT NOT NULL,
    priority INTEGER NOT NULL DEFAULT 0,
    enqueued_at INTEGER NOT NULL,
    run_id TEXT
);

CREATE INDEX IF NOT EXISTS idx_pool_queue_pool ON pool_queue(pool_name, enqueued_at);
CREATE INDEX IF NOT EXISTS idx_pool_queue_run ON pool_queue(run_id);
