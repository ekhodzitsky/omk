# src/runtime/db/ Agent Guide

Module-specific rules for the SQLite storage layer.

## Hard Constraints

1. **No raw SQL outside `schema.rs` and `migrations/`**. Business logic must not
   construct ad-hoc SQL strings. Use the repository traits.
2. **Dynamic SQL is banned**. Optional filters use fixed queries with
   `(? IS NULL OR column = ?)` — never build SQL strings with variable
   placeholder counts.
3. **Money is integer cents**. `budget_usd` and all budget checkpoint values
   are `i64` (cents). Float money is forbidden.
4. **unwrap/expect/panic are banned** in production code. Use `?`, `bail!`,
   `ok_or`, or `.context()`.
5. **File size limit: 400 lines**. Any file exceeding this must split into a
   directory module.

## Transaction Safety

- `DbTransaction` does not auto-rollback on drop in a guaranteed way.
  Best-effort rollback is spawned via `tokio::spawn`, but callers MUST
  explicitly call `commit` or `rollback`.
- `update_task_graph` is DELETE-then-INSERT. If called via `DbHandle`
  directly (auto-commit), the two statements are not atomic. Wrap in
  `DbHandle::transaction()` when atomicity matters.

## Migration Policy

- Schema version is tracked via `PRAGMA user_version`.
- `DbHandle::open` checks `user_version` and applies migrations only when
  `current < TARGET_USER_VERSION`.
- Bump `TARGET_USER_VERSION` in `handle.rs` and add new migration logic
  when changing schema.
