---
schema_version: 1
module: runtime::gates
level: subsystem
purpose: Verification gate definitions, auto-detection, and execution.
status: stable
surface:
  - name: detect_gates
    kind: fn
    visibility: pub
    contract: Scan a directory and infer verification gates from project files.
    proof:
      kind: integration-test
      target: gates_test
      command: cargo test --test gates_test
  - name: run_gates
    kind: fn
    visibility: pub
    contract: Execute all configured gates and return per-gate results.
    proof:
      kind: integration-test
      target: gates_test
      command: cargo test --test gates_test
  - name: run_gates_with_evidence
    kind: fn
    visibility: pub
    contract: Execute gates and capture structured evidence for each result.
    proof:
      kind: integration-test
      target: gates_test
      command: cargo test --test gates_test
  - name: GateDef
    kind: struct
    visibility: pub
    contract: Single gate definition (name, command, evidence config).
    proof:
      kind: integration-test
      target: gates_test
      command: cargo test --test gates_test
  - name: GateResult
    kind: struct
    visibility: pub
    contract: Outcome of a single gate execution including stdout, stderr, and pass/fail status.
    proof:
      kind: integration-test
      target: gates_test
      command: cargo test --test gates_test
  - name: VerificationConfig
    kind: struct
    visibility: pub
    contract: Collection of gates with metadata and execution ordering.
    proof:
      kind: integration-test
      target: gates_test
      command: cargo test --test gates_test
dependencies:
  internal: []
  external:
    - name: tokio
      scope: async command execution
      reason: Gates run as async shell commands.
    - name: anyhow
      scope: error handling
      reason: Result propagation for gate execution failures.
consumers:
  - path: src/cli/team/run.rs
    uses: [run_gates, GateResult]
  - path: src/runtime/goal/lifecycle.rs
    uses: [run_gates_with_evidence]
invariants:
  - id: gate-timeout-bounded
    rule: Every gate command has a configurable timeout to prevent indefinite hangs.
    proof:
      kind: static-check
      target: src/runtime/gates/run.rs
      command: cargo clippy --all-targets --all-features -- -D warnings
  - id: evidence-captured
    rule: run_gates_with_evidence always captures stdout and stderr for auditability.
    proof:
      kind: static-check
      target: src/runtime/gates/run.rs
      command: cargo clippy --all-targets --all-features -- -D warnings
verification:
  pre_change:
    - cargo test --lib runtime::gates
  full:
    - cargo test --test gates_test
    - cargo clippy --all-targets --all-features -- -D warnings
---

# runtime::gates

## Architecture

The `gates` module defines and executes verification gates.

- `detect.rs` owns heuristic gate detection from project files.
- `run.rs` owns gate execution and evidence capture.
- `types.rs` owns `GateDef`, `GateResult`, and `VerificationConfig`.

## Files

| File | Owns |
| --- | --- |
| `gates.rs` | Storefront: re-exports detect, run, and types. |
| `detect.rs` | Gate detection heuristics. |
| `run.rs` | Gate execution with timeout and evidence. |
| `types.rs` | Gate definition and result types. |
