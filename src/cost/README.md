---
schema_version: 1
module: cost
level: root
purpose: Cost estimation and persistent session cost tracking
status: stable
surface:
  - name: PricingTier
    kind: enum
    visibility: pub
    contract: Pricing categories used by the estimator.
    proof:
      kind: unit-test
      target: cost::estimator
      command: cargo test --lib cost::estimator
  - name: CostEstimate
    kind: struct
    visibility: pub
    contract: Estimation breakdown by tokens.
    proof:
      kind: unit-test
      target: cost::estimator
      command: cargo test --lib cost::estimator
  - name: estimate_cost
    kind: fn
    visibility: pub
    contract: Heuristic cost estimator. No I/O, no panics on any input.
    proof:
      kind: unit-test
      target: cost::estimator
      command: cargo test --lib cost::estimator
  - name: estimate_team_cost
    kind: fn
    visibility: pub
    contract: Quick estimate for team sessions.
    proof:
      kind: unit-test
      target: cost::estimator
      command: cargo test --lib cost::estimator
  - name: estimate_autopilot_cost
    kind: fn
    visibility: pub
    contract: Quick estimate for autopilot sessions.
    proof:
      kind: unit-test
      target: cost::estimator
      command: cargo test --lib cost::estimator
  - name: estimate_ralph_cost
    kind: fn
    visibility: pub
    contract: Quick estimate for ralph sessions.
    proof:
      kind: unit-test
      target: cost::estimator
      command: cargo test --lib cost::estimator
  - name: SessionCost
    kind: struct
    visibility: pub
    contract: Cost record for a single session.
    proof:
      kind: unit-test
      target: cost::tracker
      command: cargo test --lib cost::tracker
  - name: CostTracker
    kind: struct
    visibility: pub
    contract: Generic tracker, I/O-agnostic (parameterized by CostSink).
    proof:
      kind: unit-test
      target: cost::tracker
      command: cargo test --lib cost::tracker
  - name: CostSink
    kind: trait
    visibility: pub
    contract: Storage backend contract. Async save/load of cost records.
    proof:
      kind: unit-test
      target: cost::sink
      command: cargo test --lib cost::sink
  - name: InMemoryCostSink
    kind: struct
    visibility: pub(crate)
    contract: In-memory implementation for unit tests.
    proof:
      kind: unit-test
      target: cost::sink
      command: cargo test --lib cost::sink
  - name: JsonFileCostSink
    kind: struct
    visibility: pub
    contract: File-based implementation using atomic writes (temp + rename).
    proof:
      kind: unit-test
      target: cost::file_sink
      command: cargo test --lib cost::file_sink
dependencies:
  internal:
    - module: runtime::atomic
      scope: file_sink.rs only
      reason: JsonFileCostSink uses atomic file writes via runtime::atomic::atomic_write.
  external: []
consumers:
  - path: runtime/session.rs
    uses: [JsonFileCostSink, CostTracker, record]
  - path: cli/cost_cmd.rs
    uses: [JsonFileCostSink, CostTracker, report, clear]
invariants:
  - id: no-io-estimator
    rule: estimate_cost performs no I/O and never panics on any input.
    proof:
      kind: unit-test
      target: cost::estimator
      command: cargo test --lib cost::estimator
  - id: append-only-record
    rule: CostTracker::record appends; report is read-only.
    proof:
      kind: unit-test
      target: cost::tracker
      command: cargo test --lib cost::tracker
  - id: atomic-writes
    rule: JsonFileCostSink uses atomic writes (temp + rename).
    proof:
      kind: unit-test
      target: cost::file_sink
      command: cargo test --lib cost::file_sink
verification:
  pre_change:
    - cargo test --lib cost
  full:
    - cargo test
    - cargo clippy --all-targets --all-features -- -D warnings
---

# cost

## Architecture

```
┌─────────────────┐
│   CostTracker   │  ← pure logic (generic over CostSink)
│   <S: CostSink> │
└────────┬────────┘
         │ trait CostSink
    ┌────┴────┐
    ▼         ▼
InMemory   JsonFile
CostSink   CostSink  ← I/O adapter (uses runtime::atomic)
```

Rule: `CostTracker` knows nothing about the filesystem. All I/O is isolated in `JsonFileCostSink`.
