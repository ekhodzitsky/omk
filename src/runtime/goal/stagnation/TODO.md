# Stagnation Recovery — TODO

## Done

- [x] Core module structure (collector, detector, diagnosis, recovery, checkpoint)
- [x] Stagnation detection with sliding window and configurable thresholds
- [x] Diagnosis heuristics: TestFlakiness, ScopeTooLarge, ExternalDependencyBroken, CircularFix, InefficientExploration, Unknown
- [x] Recovery planning with risk levels and estimated token costs
- [x] Checkpoint save/load/rollback roundtrip
- [x] CLI commands: `omk goal diagnose`, `omk goal recover`, `omk goal rollback`
- [x] Config parsing (`StagnationConfig`, `StagnationThresholdsConfig`)
- [x] Schema changes: `GoalState.recovery_attempts`, `Task.recovery_parent`
- [x] Unit tests for all heuristics and checkpoint roundtrip
- [x] `#[source]`-preserving error types

## In Progress / Known Gaps

- [ ] **Coverage delta metric** requires CI-065 (Coverage Radar). Currently always `None` and explicitly excluded from stagnation counting.
- [ ] **ReviewRejectionLoop** heuristic is a placeholder. Needs review task history access.
- [ ] **Automatic lifecycle integration**: Detection is CLI-only. Should run as background task after each goal iteration (between Verification and Planning).
- [ ] **Persisted history file**: `StagnationCollector::save/load` exists but is not called by the lifecycle. CLI `diagnose` uses it when available.
- [ ] **Recovery task creation**: `cmd_recover` prints a placeholder instead of actually spawning recovery tasks in the task graph.
- [ ] **Event emission**: Stagnation detection, diagnosis, and checkpoint creation should emit structured events to the goal event log.

## Future

- [ ] Add `MetricsBuilder` to replace `build_metrics()`'s 7 parameters
- [ ] Unify `StagnationThresholds` and `StagnationThresholdsConfig` via `From` impl (done in detector, needs config side)
- [ ] Integration tests for CLI commands (mock goal state + temp dir)
- [ ] Config deserialization tests for `StagnationConfig`
