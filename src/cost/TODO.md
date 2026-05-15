# cost — TODO

## Current (module pilot)
- [x] Extract `CostSink` trait for storage abstraction.
- [x] Make `CostTracker` generic over `CostSink`.
- [x] Add `InMemoryCostSink` for unit tests.
- [x] Move I/O into `JsonFileCostSink`.
- [x] Add unit tests for `estimator`.
- [x] Add unit tests for `CostTracker` with `InMemoryCostSink`.

## Next
- [ ] Property-based tests: edge cases for `estimate_cost` (zero duration, huge worker_count).
- [ ] Budget limit: `CostTracker::check_budget(limit) -> bool`.
- [ ] Integration with `runtime/goal/budget/` — provide `CostEstimate` as input for goal budgets.
- [ ] Streaming events: notify `vis/` of limit breaches in real time.
- [ ] Serialize to JSONL instead of JSON array for append-only semantics.
