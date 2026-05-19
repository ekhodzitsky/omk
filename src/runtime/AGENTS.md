# runtime — Agent Guide

## Editing Rules

1. **Async workers need explicit ownership.** Every spawned task, child process,
   Wire worker, scheduler loop, or background watcher must document in code who
   cancels it, who joins/aborts it, and what event evidence is emitted on
   stop/failure.
2. **State mutations are atomic.** Use `atomic_write` for all JSONL, JSON, and
   TOML state files. Never write directly to the final path.
3. **I/O happens at the edge.** `runtime/gates/` uses a `GateRunner` trait.
   `runtime/scheduler/` uses `SchedulerBackend`. Pure logic (verification
   analysis, task ordering) must not depend on `tokio::fs` or `Command`.
4. **Events are append-only.** `EventSink` is the only way to record runtime
   events. Do not write events through ad-hoc `fs::write` calls.
5. **Test through mocks.** `MockEventSink`, `MockCostSink`, and `MockWireClient`
   live in `src/test_helpers.rs`. Unit tests for runtime logic use these mocks;
   integration tests verify end-to-end behavior with real I/O.
