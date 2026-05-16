---
schema_version: 1
module: runtime::ralph
level: subsystem
purpose: Ralph persistent loop — PRD generation, verify/fix cycles with Kimi CLI.
status: pilot
surface:
  - name: run_ralph
    kind: fn
    visibility: pub
    contract: Execute the Ralph loop (generate PRD, run Kimi, run tests, fix failures) until passing or max iterations.
    proof:
      kind: integration-test
      target: ralph_test
      command: cargo test --test ralph_test
  - name: generate_prd
    kind: fn
    visibility: pub
    contract: Generate a Prd struct from a free-form task description.
    proof:
      kind: unit-test
      target: runtime::ralph
      command: cargo test --lib runtime::ralph
  - name: run_kimi
    kind: fn
    visibility: pub
    contract: Spawn Kimi CLI with a prompt and return captured stdout.
    proof:
      kind: integration-test
      target: ralph_test
      command: cargo test --test ralph_test
  - name: run_tests
    kind: fn
    visibility: pub
    contract: Run the project's test suite and return pass/fail status.
    proof:
      kind: integration-test
      target: ralph_test
      command: cargo test --test ralph_test
  - name: state_dir_for
    kind: fn
    visibility: pub
    contract: Resolve the Ralph state directory for a given task.
    proof:
      kind: unit-test
      target: runtime::ralph
      command: cargo test --lib runtime::ralph
dependencies:
  internal:
    - module: agents
      scope: context injection
      reason: Injects AGENTS.md context into Ralph prompts.
    - module: notifications
      scope: event emission
      reason: Emits RalphComplete notification on finish.
  external:
    - name: tokio
      scope: async process I/O
      reason: Spawns Kimi CLI and test commands asynchronously.
    - name: anyhow
      scope: error handling
      reason: Result propagation across the Ralph loop.
consumers:
  - path: src/cli/ralph.rs
    uses: [run_ralph, generate_prd]
invariants:
  - id: max-iterations-bounded
    rule: The Ralph loop terminates after a configurable maximum number of iterations.
    proof:
      kind: static-check
      target: src/runtime/ralph/engine.rs
      command: cargo clippy --all-targets --all-features -- -D warnings
verification:
  pre_change:
    - cargo test --lib runtime::ralph
  full:
    - cargo test --test ralph_test
    - cargo clippy --all-targets --all-features -- -D warnings
---

# runtime::ralph

## Architecture

The `ralph` module implements a persistent verify/fix loop.

- `engine.rs` owns `run_ralph` and the main loop orchestration.
- `generate.rs` owns PRD generation from a task description.
- `runner.rs` owns `run_kimi` and `run_tests` helpers.
- `progress.rs` owns progress tracking during the loop.

## Files

| File | Owns |
| --- | --- |
| `mod.rs` | Storefront: re-exports `run_ralph`, `generate_prd`, `run_kimi`, `run_tests`, and `state_dir_for`. |
| `engine.rs` | Ralph loop engine and state management. |
| `generate.rs` | PRD generation logic. |
| `runner.rs` | Kimi CLI and test suite runners. |
| `progress.rs` | Progress tracking during the loop. |
