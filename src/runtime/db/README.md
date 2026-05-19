# src/runtime/db/

SQLite-backed durable state management for `omk goal`.

## Purpose

Provides a self-contained storage layer over SQLite. This module knows about
schemas, migrations, transactions, and WAL mode. It does **not** know about
goal lifecycles, task graphs, proof semantics, or the wire protocol.

All state previously stored in JSON files (`goal.json`, `task-graph.json`,
`events.jsonl`, `proof.json`, etc.) is consolidated into a single SQLite
database.

## Public API

### Entry point

- `DbHandle::open(path)` — open or create a database, apply migrations, enable WAL.
- `DbHandle::transaction()` — begin an explicit transaction.
- `DbHandle::backup_to(dest)` — backup via the SQLite online backup API.

### Repositories (traits + concrete impls)

Each repository is exposed both as a method on `DbHandle` (auto-commit) and
via `DbTransaction` (participates in the active transaction).

| Trait        | Key operations                              |
|--------------|---------------------------------------------|
| `GoalRepo`   | `create`, `get`, `update_status`, `list`, `delete` |
| `TaskRepo`   | `create_batch`, `get_by_goal`, `update_status`, `update_task_graph`, `delete_by_goal` |
| `EventRepo`  | `append`, `get_by_goal`, `delete_by_goal`   |
| `ProofRepo`  | `upsert`, `get`, `delete`                   |
| `BudgetRepo` | `append_checkpoint`, `get_by_goal`, `delete_by_goal` |
| `ArtifactRepo` | `register`, `get_by_goal`, `delete_by_goal` |

### Types

- `GoalRecord`, `TaskRecord`, `EventRecord`, `ProofRecord`
- `BudgetCheckpoint`, `ArtifactRecord`
- `GoalFilter`, `GoalSummary`
- `DbError`

## Status

- Schema: v1 (initial)
- Migrations: applied automatically on `DbHandle::open`
- WAL mode: enabled
- Foreign keys: enabled
- Test coverage: 18 integration tests covering CRUD, transactions, concurrency,
  cascading deletes, backups, and idempotent migrations.

## Dependencies

| Crate            | Version | Purpose                       |
|------------------|---------|-------------------------------|
| `tokio-rusqlite` | 0.5     | Async SQLite connection       |
| `rusqlite`       | 0.30    | Sync SQLite driver + backup   |
| `chrono`         | —       | Timestamps (project-wide)     |
| `thiserror`      | —       | Error derives (project-wide)  |
| `tempfile`       | —       | Test temp directories         |

## File map

```
src/runtime/db/
  mod.rs          — public re-exports, DbHandle repo accessors
  handle.rs       — DbHandle: open, migrate, transaction, backup
  transaction.rs  — DbTransaction: commit, rollback, repo accessors
  schema.rs       — SQL DDL as const strings
  migrations/
    001_initial.sql
  error.rs        — DbError enum
  types.rs        — record structs and filters
  repo/
    mod.rs        — re-exports
    goal.rs       — GoalRepo trait + impl
    task.rs       — TaskRepo trait + impl
    event.rs      — EventRepo trait + impl
    proof.rs      — ProofRepo trait + impl
    budget.rs     — BudgetRepo trait + impl
    artifact.rs   — ArtifactRepo trait + impl
  tests.rs        — integration tests (18 tests)
```

## Invariants

- `DbTransaction` does **not** auto-rollback on drop. Callers must explicitly
  invoke `commit` or `rollback`.
- All repo operations through `DbTransaction` participate in the same SQLite
  transaction because all clones share the same underlying connection.
- Foreign keys are enforced; deleting a goal cascades to tasks, events, proofs,
  budget checkpoints, and artifacts.
- `unwrap()`/`expect()` are banned in production code; used only in tests.
