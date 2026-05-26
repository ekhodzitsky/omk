# TODO — runtime

## Active
- [ ] `runtime/ask/` needs contract tests for each provider command path.
- [ ] `runtime/autopilot/` needs unit tests for phase transition logic.
- [ ] `runtime/gates/` needs schema validation for `.gates.toml` files.
- [ ] `runtime/gates/circuit_breaker.rs` needs integration tests with real SQLite repo.
- [ ] `runtime/proof/` needs golden tests for proof JSON output shape.
- [ ] `runtime/scheduler/pool.rs` needs in-memory simulation tests for admission races.
- [ ] `runtime/goal/stagnation/` needs automatic lifecycle integration (background task after each iteration).
- [ ] `runtime/goal/stagnation/` needs event emission to the goal event log.
- [ ] `runtime/goal/stagnation/` needs integration tests for CLI commands (mock goal state + temp dir).

## Later
- Consolidate shared retry/backoff logic between `ralph`, `autopilot`, and `scheduler`.
- Unify `StagnationThresholds` and `StagnationThresholdsConfig` via `From` impl on config side.
- Add `MetricsBuilder` to replace `build_metrics()`'s 7 parameters.
