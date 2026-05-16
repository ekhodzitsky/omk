---
schema_version: 1
module: runtime::ask
level: subsystem
purpose: Multi-provider LLM query execution with artifact saving and synthesis.
status: stable
surface:
  - name: ask_single
    kind: fn
    visibility: pub
    contract: Query a single provider by name and return the raw response string.
    proof:
      kind: unit-test
      target: runtime::ask
      command: cargo test --lib runtime::ask
  - name: ask_providers
    kind: fn
    visibility: pub
    contract: Query multiple named providers concurrently and return their responses.
    proof:
      kind: unit-test
      target: runtime::ask
      command: cargo test --lib runtime::ask
  - name: ask_all
    kind: fn
    visibility: pub
    contract: Query all installed providers concurrently, optionally saving artifacts.
    proof:
      kind: unit-test
      target: runtime::ask
      command: cargo test --lib runtime::ask
  - name: synthesize
    kind: fn
    visibility: pub
    contract: Build a synthesis prompt from provider outputs and query a synthesizer.
    proof:
      kind: unit-test
      target: runtime::ask
      command: cargo test --lib runtime::ask
  - name: run_advisor_direct
    kind: fn
    visibility: pub
    contract: Run a single provider advisor with a prompt and timeout.
    proof:
      kind: unit-test
      target: runtime::ask
      command: cargo test --lib runtime::ask
  - name: ALL_PROVIDERS
    kind: const
    visibility: pub
    contract: Static list of supported provider names.
    proof:
      kind: static-check
      target: src/runtime/ask/provider.rs
      command: cargo check
dependencies:
  internal: []
  external:
    - name: tokio
      scope: async process spawning
      reason: Spawns provider CLI child processes concurrently.
    - name: anyhow
      scope: error handling
      reason: Result propagation across async boundaries.
consumers:
  - path: src/cli/ask.rs
    uses: [ask_single, ask_all, synthesize]
  - path: src/runtime/ralph/runner.rs
    uses: [run_advisor_direct]
invariants:
  - id: provider-timeout-bounded
    rule: Direct advisor execution uses a configurable timeout to prevent hung processes.
    proof:
      kind: static-check
      target: src/runtime/ask/execution.rs
      command: cargo clippy --all-targets --all-features -- -D warnings
  - id: no-panic
    rule: Public functions do not panic; errors are propagated via Result.
    proof:
      kind: static-check
      target: src/runtime/ask
      command: cargo clippy --all-targets --all-features -- -D warnings
verification:
  pre_change:
    - cargo test --lib runtime::ask
  full:
    - cargo test
    - cargo clippy --all-targets --all-features -- -D warnings
---

# runtime::ask

## Architecture

The `ask` module is a thin multi-provider query layer.

- `api.rs` owns `ask_single`, `ask_providers`, and `ask_all`.
- `provider.rs` owns provider discovery and command building.
- `execution.rs` owns direct advisor spawning and outbox polling.
- `synthesis.rs` owns multi-response synthesis.
- `artifact.rs` owns artifact directory paths and saving.

## Files

| File | Owns |
| --- | --- |
| `mod.rs` | Storefront: re-exports public surface. |
| `api.rs` | High-level ask functions. |
| `provider.rs` | Provider list, installation checks, command building. |
| `execution.rs` | Direct advisor execution and outbox polling. |
| `synthesis.rs` | Prompt synthesis across multiple provider outputs. |
| `artifact.rs` | Artifact directory management and saving. |
