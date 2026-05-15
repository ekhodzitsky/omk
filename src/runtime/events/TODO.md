# TODO — runtime::events

## Current
- [ ] Add typed payloads for the remaining event kinds that currently use raw `serde_json::json!` or have no payload struct:
  - `WorkerStalled`, `WorkerDead`, `WorkerRecovered`
  - `TaskProposed`, `TaskAccepted`, `TaskRejected`, `TaskStarted`, `TaskOutput`, `TaskFailed`
  - `RetryScheduled`, `ManualInterrupt`
  - `GoalPaused`, `GoalResumed`, `GoalBudgetExhausted`, `GoalBudgetExtended`, `BudgetCheckpoint`
- [ ] Document why `payload_string` falls back to `value.get("0")` (wrapped identifier objects) or replace with an explicit accessor.
- [ ] Attach `kill_on_drop(true)` or `CancellationToken` to the `JsonlWriter` actor task to comply with async worker ownership rules.

## Next
- [ ] Consider adding `EventReader::read_last_n` for tail-style queries (HUD live view, replay).
- [ ] Evaluate whether `EventReader` should cache file handles for high-frequency polling consumers.
- [ ] Add golden file tests for the JSONL output format to protect downstream parsers.
