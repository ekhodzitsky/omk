---
schema_version: 1
module: runtime::autopilot
level: subsystem
purpose: 6-phase autopilot state machine (plan, code, test, verify, review, deliver).
status: pilot
surface:
  - name: Autopilot
    kind: struct
    visibility: pub
    contract: Core autopilot engine owning phase state, logs, and execution loop.
    proof:
      kind: integration-test
      target: autopilot_test
      command: cargo test --test autopilot_test
  - name: run_autopilot
    kind: fn
    visibility: pub
    contract: Start a new autopilot run from a project directory.
    proof:
      kind: integration-test
      target: autopilot_test
      command: cargo test --test autopilot_test
  - name: resume_autopilot
    kind: fn
    visibility: pub
    contract: Resume an existing autopilot run from saved state.
    proof:
      kind: integration-test
      target: autopilot_test
      command: cargo test --test autopilot_test
  - name: AutopilotState
    kind: struct
    visibility: pub
    contract: Serializable snapshot of autopilot phase, QA results, and validation.
    proof:
      kind: integration-test
      target: autopilot_test
      command: cargo test --test autopilot_test
dependencies:
  internal:
    - module: notifications
      scope: event emission
      reason: Emits AutopilotComplete and AutopilotFailed notifications.
  external: []
consumers:
  - path: src/cli/autopilot.rs
    uses: [run_autopilot, resume_autopilot]
invariants:
  - id: phase-log-append-only
    rule: PhaseLog entries are append-only during a run.
    proof:
      kind: static-check
      target: src/runtime/autopilot/types.rs
      command: cargo clippy --all-targets --all-features -- -D warnings
verification:
  pre_change:
    - cargo test --lib runtime::autopilot
  full:
    - cargo test --test autopilot_test
    - cargo clippy --all-targets --all-features -- -D warnings
---

# runtime::autopilot

## Architecture

The `autopilot` module runs a 6-phase pipeline over a project directory.

- `cli.rs` owns `run_autopilot` and `resume_autopilot` entrypoints.
- `engine/` owns the `Autopilot` struct and phase loop.
- `types.rs` owns `AutopilotState`, `PhaseLog`, and phase enums.
- `helpers.rs` owns utility functions for the engine.

## Files

| File | Owns |
| --- | --- |
| `autopilot.rs` | Storefront: re-exports `Autopilot`, `run_autopilot`, `resume_autopilot`, and types. |
| `cli.rs` | CLI entrypoints for starting and resuming autopilot. |
| `engine/` | Core autopilot engine and phase execution. |
| `types.rs` | Autopilot state, phase, and result types. |
| `helpers.rs` | Helper utilities for the engine. |
