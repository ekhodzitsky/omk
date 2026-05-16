---
schema_version: 1
module: runtime::proof
level: subsystem
purpose: Proof/readiness report data model and synthesis for team runs.
status: stable
surface:
  - name: Proof
    kind: struct
    visibility: pub
    contract: Aggregated readiness report with status, changed files, gate results, and evidence.
    proof:
      kind: integration-test
      target: proof_cmd_test
      command: cargo test --test proof_cmd_test
  - name: ProofStatus
    kind: enum
    visibility: pub
    contract: Readiness state — Ready, NotReady, or Failed.
    proof:
      kind: integration-test
      target: proof_cmd_test
      command: cargo test --test proof_cmd_test
  - name: ProofGate
    kind: struct
    visibility: pub
    contract: Gate evidence embedded in a proof report.
    proof:
      kind: integration-test
      target: proof_cmd_test
      command: cargo test --test proof_cmd_test
  - name: ChangedFile
    kind: struct
    visibility: pub
    contract: File change entry with path and operation (created/modified/deleted).
    proof:
      kind: integration-test
      target: proof_cmd_test
      command: cargo test --test proof_cmd_test
dependencies:
  internal:
    - module: runtime::events
      scope: evidence summarization
      reason: Proof synthesis reads event logs for gate and wire evidence.
  external:
    - name: serde
      scope: serialization
      reason: Proof types derive Serialize/Deserialize for JSON output.
    - name: chrono
      scope: timestamps
      reason: Proof includes creation timestamps.
consumers:
  - path: src/cli/proof_cmd.rs
    uses: [Proof, ProofStatus]
  - path: src/cli/team/proof.rs
    uses: [Proof, ProofStatus, ProofGate]
  - path: src/runtime/scheduler/runner.rs
    uses: [Proof]
invariants:
  - id: proof-immutable
    rule: Proof structs are immutable snapshots; no mutation methods are exposed.
    proof:
      kind: static-check
      target: src/runtime/proof/types.rs
      command: cargo clippy --all-targets --all-features -- -D warnings
verification:
  pre_change:
    - cargo test --lib runtime::proof
  full:
    - cargo test --test proof_cmd_test
    - cargo clippy --all-targets --all-features -- -D warnings
---

# runtime::proof

## Architecture

The `proof` module produces readiness reports for team runs.

- `types.rs` owns the `Proof`, `ProofStatus`, `ProofGate`, and `ChangedFile` data model.
- `generator/` owns proof synthesis from events and gate results.

## Files

| File | Owns |
| --- | --- |
| `proof.rs` | Storefront: re-exports generator and types. |
| `types.rs` | Proof data model and status enums. |
| `generator/` | Proof synthesis from runtime events and evidence. |
