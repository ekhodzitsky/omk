# oh-my-kimi Agent Guide

This file contains agent-level conventions for the entire project tree.
For OMK-specific context (wire protocol, agent roles, roadmap), see `.omk/AGENTS.md`.
For the main product direction, read `SPEC.md`, `ROADMAP.md`, and `TODO.md`.
For market positioning and competitor boundaries, read
`docs/COMPETITIVE_POSITIONING.md`.

## Contents

- [Meta Principle](#meta-principle)
- [Behavioral Guidelines](#behavioral-guidelines)
- [Project Contract Rules](#project-contract-rules-hard-constraints)
- [Release Discipline](#release-discipline-hard-constraints)
- [Multi-Agent Development Protocol](#multi-agent-development-protocol-hard-constraints)
- [Agent Module Architecture](#agent-module-architecture-hard-constraints)
- [Rust Safety Rules](#rust-safety-rules-hard-constraints)
- [Async & Concurrency Architecture](#async--concurrency-architecture-hard-constraints)
- [Data Layer & State Management](#data-layer--state-management-hard-constraints)
- [Error Handling Doctrine](#error-handling-doctrine-hard-constraints)
- [Security & Trust Boundaries](#security--trust-boundaries-hard-constraints)
- [Observability Standards](#observability-standards-hard-constraints)
- [SOTA Testing](#sota-testing)
- [Build & Test](#build--test)
- [Clippy Lint Policy](#clippy-lint-policy)
- [Editing Rules](#editing-rules)

## Meta Principle

Before applying any rule or refactor, ask: **what problem does this solve?**
A newtype, a refactor, or an abstraction is justified only if it prevents a concrete bug,
clarifies an invariant, or removes a footgun. If the answer is "it looks better" — revert.
Decoration is not engineering.

## Behavioral Guidelines

- **State assumptions explicitly.** If uncertain, ask before implementing.
- **Minimum code.** No speculative abstractions. No features beyond the request. If you write 200 lines and it could be 50, rewrite.
- **Surgical changes only.** Touch only what you must. Match existing style. Remove imports/variables made unused by *your* changes; don't delete pre-existing dead code unless asked.
- **Goal-driven execution.** Every task needs verifiable success criteria and a brief plan (`Step → verify: check`).
- **Prefer `?` over `unwrap`/`expect` even in tests** where it keeps the test readable.

## Project Contract Rules (Hard Constraints)

These rules protect OMK's most fragile cross-cutting contracts. Domain-specific rules (async, errors, security, testing, observability) live in their own sections below.

`omk goal` is the north-star feature. It must be designed as a proof-driven
controller over existing Wire/team/event/proof primitives, not as an unbounded
recursive agent launcher.

1. **`src/main.rs` stays thin.** The binary crate must only call the library
   entrypoint (`omk::cli::run().await`). No command dispatch or business logic in the binary crate.
2. **Public API is opt-in.** Prefer `pub(crate)`. New `pub` items require a concrete external caller or a paved-path rationale, plus an integration test.
3. **Protocol facts must not go stale.** If `KIMI_WIRE_PROTOCOL_VERSION`, event names, request names, or Kimi CLI behavior changes, update `.omk/AGENTS.md`, README/docs, changelog, and tests in the same change set. See also SOTA Testing Tier 2 (property tests) for Wire compat.
4. **Events, metrics, and proof fields are public API.** Renames/removals need a compatibility alias, migration note, or explicit changelog entry explaining the break. See also Observability Standards §4 for metric naming.
5. **Dependencies are architecture changes.** No new crate without a rationale: why std/local code is not enough, transitive impact, MSRV, license, and feature-flag consequences. See also Security §8 for supply-chain requirements. Small helper crates are rejected by default.
6. **Refactors isolate mechanics from behavior.** File moves, splits, renames, and formatting-only changes must be separate from semantic changes whenever practical.

## Release Discipline (Hard Constraints)

Releases are not a separate batch task. They are the cumulative
result of every PR following the same rules. CHANGELOG drift,
README staleness, and missing version bumps are the leading cause
of "publish-day debt" that blocks crates.io cuts. Discipline is
enforced per-PR.

> Cross-references: [Project Contract Rules](#project-contract-rules-hard-constraints)
> §3 (protocol facts staleness), §4 (events/metrics/proof public API);
> [Multi-Agent Protocol](#multi-agent-development-protocol-hard-constraints)
> §6 (PR evidence).

### 1. CHANGELOG-per-PR

Every PR that ships a user-visible change must add an entry to
`CHANGELOG.md` under the `## [Unreleased]` heading **in the same
PR** as the change itself. Acceptable:

- `### Added` / `### Changed` / `### Fixed` / `### Removed`
- `### Wire Protocol` (for protocol-specific changes)
- `### Dependencies` (for dep bumps)

"User-visible" means: CLI surface change (new command/flag/
output/exit code), public Rust API change, file format change,
event/metric/proof field change, security policy change,
behavior change observable from outside the binary.

Internal-only changes (refactor without API change, test-only,
CI hygiene, doc-only) do NOT require a `[Unreleased]` entry. The
PR template box marks the choice.

**Why:** retroactive CHANGELOG reconstruction at release time
misses ~30 % of user-visible changes (historical observation).
Per-PR entries are the only reliable record.

### 2. Documentation Sync

When a PR changes user-facing surface, associated docs must
change in the same PR:

- `README.md` — if entry point / install / Quick Start affected.
- `docs/<topic>.md` — if a topic doc covers the surface.
- `src/<module>/README.md` and/or `src/<module>/AGENTS.md` — if
  module-level public API changes (per §Agent Module Architecture).
- `SPEC.md` / `ROADMAP.md` — if north-star / staging shifts.

Reviewer must check the PR template Documentation checkboxes;
unchecked N/A boxes with no explanation are grounds for review
rejection.

### 3. Version Bump Policy (pre-1.0)

OMK is `0.x.y`. While pre-1.0:

- **Patch bump** (`0.x.y → 0.x.y+1`): bug fix, internal refactor,
  no surface change. Multiple patch-level PRs may accumulate
  between releases.
- **Minor bump** (`0.x.y → 0.(x+1).0`): new feature, possibly
  breaking. Wire protocol bumps qualify (protocol version
  history kept in CHANGELOG).
- **Major bump** (`0.x.y → 1.0.0`): commits to API stability.
  Requires explicit owner decision and a clean migration plan
  from 0.x.

The version field in `Cargo.toml` and `VERSION` file are bumped
by the release PR, not per feature PR. Per-feature PRs only add
to `## [Unreleased]`.

### 4. Release Cut Procedure

A release is a dedicated PR that:

1. Bumps `VERSION` and `Cargo.toml` `[package].version`.
2. Replaces `## [Unreleased]` with `## [X.Y.Z] - YYYY-MM-DD` in
   `CHANGELOG.md`, then opens a fresh empty `## [Unreleased]`
   block.
3. Runs `cargo publish --dry-run --allow-dirty` and pastes the
   result in the PR body.
4. After merge, the orchestrator tags `vX.Y.Z` and (if applicable)
   runs `cargo publish`.

Release PRs do NOT introduce new features or fixes. If the
[Unreleased] section needs editing for clarity, that is OK; if
it needs adding new bullets, those bullets belong in separate
feature PRs that merge before the release PR.

### 5. Backward-Compat Horizon

See [Data Layer §1](#data-layer--state-management-hard-constraints) for
on-disk format compatibility. See §Project Contract #4 for
events/metrics/proof. Both rules apply per-PR, not per-release.

### Migration Path

PRs already in flight at the time this section is introduced are
exempt from the per-PR CHANGELOG requirement only until they
merge. New PRs opened after this section lands must comply.

## Multi-Agent Development Protocol (Hard Constraints)

OMK is expected to be edited by multiple agents and humans at the same time
(Codex, Kimi, Claude, and future `omk goal` workers). Coordination is part of
correctness, not ceremony.

1. **`master` / `main` are read-only.** Do not commit or push feature work
   directly to protected base branches. All changes land through PRs.
2. **One owned slice per branch/worktree.** Before editing, pick a concrete
   task or subgoal, name the owner, and declare the intended write scope.
3. **Use isolated worktrees for parallel work.** Independent agents should work
   from separate git worktrees or branches rooted at the protected baseline.
4. **Branches are task-scoped.** Use branch names such as
   `agent/<task-slug>`, `codex/<task-slug>`, `kimi/<task-slug>`, or
   `claude/<task-slug>`.
5. **Write scopes are explicit.** The PR must list owned files/modules. If two
   agents need overlapping files, serialize the work or create an integrator
   PR instead of racing the edit.
6. **PRs carry evidence.** PR bodies must include task/scope, owner, risks,
   verification output, known gaps, and any handoff notes.
7. **External trackers are optional.** GitHub Issues or another tracker
   may be used for long-running coordination, but they are not required for
   normal development and must not become a hard runtime dependency.
8. **Goal workers inherit the same rule.** Future `omk goal` execution records
   task ownership in its local task graph, writes changes in task-scoped
   worktrees, and delivers repository mutations through PRs before treating
   them as integrated.

## Agent Module Architecture (Hard Constraints)

OMK is developed by multiple agents working in parallel. Each agent must be
able to own a module without understanding the implementation of its neighbors.
This requires explicit contracts, local documentation, and trait-based
boundaries.

### Module as an Agent Context

Every top-level module under `src/X/` is an **agent context**: a self-contained
unit of work that an agent can bring to state-of-art independently.

Required files in every `src/X/`:
- **`README.md`** — purpose, public API, status, dependencies on other modules.
- **`TODO.md`** — current tasks, known gaps, planned features.
- **`AGENTS.md`** (if module-specific rules exist) — editing constraints,
  invariants, or safety rules that differ from the root.
- **Unit tests** — every public type and function must have deterministic
  `#[cfg(test)]` coverage. Prefer in-memory trait mocks over integration tests
  for internal logic.

### Contract-Driven Boundaries

Modules communicate through **trait contracts**, not concrete types.

Rules:
1. **I/O happens at the edge.** Pure logic must not depend on `tokio::fs`,
   `reqwest`, child processes, or other I/O directly. Wrap I/O in a trait and
   keep the trait in the same module as the consumer.
2. **No silent cross-layer coupling.** `runtime/` must not import `cost/`,
   `notifications/`, `vis/`, or `cli/`. The only exception is `runtime/session.rs`,
   which is scheduled for removal into `cli/`.
3. **Trait per boundary.** Every module that stores state, sends network
   traffic, or spawns processes must expose a trait for its behavior.
   Example: `CostSink`, `EventSink`, `WireClient`.
4. **Tests use mocks.** Unit tests must exercise logic through the trait
   interface with an in-memory mock. Do not require temp files or shell
   scripts for unit tests.

### Documentation as System Prompt

README/TODO/AGENTS.md inside a module are the **system prompt** for the agent
assigned to that module. They must answer:
- What does this module do?
- What is its public API (types, traits, functions)?
- Who are its consumers (which modules call it)?
- What are the invariants and preconditions?
- What is the current state of completion?

### Migration Path

Not all modules comply yet. When editing a module, bring it into compliance
with this section before adding new features. Do not retrofit the entire
codebase in one PR — migrate module-by-module, starting with the one you are
touching.

## Rust Safety Rules (Hard Constraints)

These rules apply to **new or modified production code** under `src/` (outside
`#[cfg(test)]`). Violations in touched code must be fixed before merge; older
legacy violations should be cleaned in focused follow-up changes rather than
hidden under unrelated diffs.

1. **`unwrap()` is banned.** Use `?`, `if let`, `match`, `ok_or`, `bail!`, or `.context()`.
2. **`expect()` is banned.** No "this should never happen" — it always happens eventually.
3. **`panic!()` is banned.** Graceful degradation only; propagate errors via `Result`.
4. **`std::thread::sleep` is banned in `async fn`.** Use `tokio::time::sleep(...).await`.
5. **`std::sync::Mutex` is banned in `async fn`.** Use `tokio::sync::Mutex` to avoid blocking the executor.
6. **All external `Command::output().await` must have a `tokio::time::timeout`.** Prevent infinite hangs from rogue child processes.
7. **All `spawn()` calls must set `kill_on_drop(true)` or attach to a `CancellationToken`.** Prevent zombie processes.

### Preconditions & Invariants

Prefer expressing preconditions in types before comments or runtime checks:

1. Use a specific type/newtype/parser constructor that makes invalid states
   unrepresentable.
2. Keep fields private when they carry invariants.
3. If the invariant cannot be encoded in the type, document it and add a
   `debug_assert!` next to the use.

Example:

```rust
/// Average of a non-empty slice.
/// Precondition: `!items.is_empty()`
pub fn average(items: &[f64]) -> f64 {
    debug_assert!(!items.is_empty(), "precondition: non-empty slice");
    items.iter().sum::<f64>() / items.len() as f64
}
```

`debug_assert!` is a last-line development check, not a substitute for type
design.

### Tests (`#[cfg(test)]`)

`unwrap()`/`expect()` are allowed for brevity, but prefer `?` where it keeps the test readable.

## Async & Concurrency Architecture (Hard Constraints)

> Cross-references: [Rust Safety Rules](#rust-safety-rules-hard-constraints) (spawn rules, Mutex), [Error Handling Doctrine](#error-handling-doctrine-hard-constraints) §6 (async error paths), [Observability Standards](#observability-standards-hard-constraints) §1 (span ownership).

These rules govern the async runtime layer: Wire workers, approval proxy, event delivery, gate execution, and the TUI event loop. The goal is **structured concurrency** — every concurrent task has a visible owner, a defined cancellation path, and a guaranteed join/abort point.

### Meta Principle

**Prefer message passing over shared mutable state. Prefer structured tasks over fire-and-forget spawns. Prefer graceful degradation over panic.**

### 1. Task Ownership & Lifecycle

Every spawned task is owned. Ownership must be visible in code shape.

- **Parent must await or manage the `JoinHandle`.** Use `tokio::task::JoinSet` for groups of related tasks. Avoid bare `tokio::spawn` without storing the handle.
- **Document the owner.** In a code comment near the spawn, state: who cancels this task, who joins it, and under what condition it stops.
- **No fire-and-forget unless daemon.** A task that outlives its creator must be explicitly declared as a daemon in a `static` or top-level scope, with its own cancellation root.
- **Child processes are tasks too.** `Command` spawns must set `kill_on_drop(true)` (already required in Rust Safety Rules) and the owning task must await the `Child` handle.

**Rationale:** Unowned tasks are memory leaks that happen to execute. `JoinSet` and explicit handles are the standard in Tokio 1.40+ and the foundation for deterministic shutdown.

### 2. Cancellation & Shutdown

All long-running tasks must be cancellable cooperatively.

- **Root every cancellable scope in a `CancellationToken`.** Pass `child_token()` to sub-tasks. Never clone the root token directly to children.
- **Check cancellation in every loop.** A task with a loop must `select!` on the token at least once per iteration, or call `token.cancelled().await` before blocking work.
- **Cancellation is not abortion.** On cancellation, flush buffers, close files, emit a final event/proof record, and drop locks. Do not leave temp files or half-written JSON.
- **Graceful shutdown has a timeout.** Shutdown sequence: stop accepting work → drain queues → join tasks → exit. If graceful shutdown exceeds its budget (default 30 s), abort remaining tasks and log a warning.

**Rationale:** Cooperative cancellation is the only safe way to stop async Rust. `AbortHandle` aborts at await points but does not clean up; `CancellationToken` lets the task exit cleanly. AWS Lambda, Tokio itself, and Axum use this pattern.

### 3. Channels & Backpressure

Channels are the primary concurrency primitive. They must be bounded and sized intentionally.

- **All channels are bounded.** Every `mpsc::channel(n)` must have an explicit capacity `n > 0`. The capacity is part of the architecture, not an afterthought.
- **Document capacity.** In a comment near the channel creation, explain why this capacity: e.g., `// bound = 64: max concurrent wire events per worker`.
- **Backpressure over drop.** When a bounded channel is full, the sender must wait (`await send(...)`), not drop messages. Dropping is permitted only for explicitly documented telemetry/metrics channels.
- **Prefer `mpsc` over `broadcast`.** Use `broadcast` only for true publish-subscribe (e.g., TUI event bus). Remember that `broadcast` receivers lag and may drop messages if slow.
- **Close the sender to signal EOF.** When a producer is done, drop the sender. Consumers must handle `None` from `recv()` as a clean termination signal, not an error.

**Rationale:** Unbounded channels hide memory leaks. Backpressure propagates load upstream instead of buffering indefinitely. This is the standard in every production Tokio deployment (Discord, Cloudflare, AWS).

### 4. Concurrency Primitives

Locks and atomics are last resorts after channels and scoped tasks.

- **`tokio::sync::Mutex` in async code.** Never `std::sync::Mutex` across `.await` (Rust Safety Rule #5). Even `tokio::sync::Mutex` must be held for the **shortest possible scope** — never across I/O or channel operations.
- **`tokio::sync::RwLock` for read-heavy shared state.** Prefer multiple readers over cloning large structures.
- **Atomics only when benchmarked.** Use `AtomicU64` etc. only when a `Mutex<u64>` is proven to be a bottleneck. Default ordering is `SeqCst`; relax only with evidence and a comment explaining the memory model reasoning.
- **`Rc` and `RefCell` are banned across spawn boundaries.** Any type that crosses a task boundary must be `Send`. Use `Arc` and atomics or channels instead.

**Rationale:** Holding a lock across `.await` serializes the executor and destroys parallelism. Atomics are easy to misuse without memory-model justification. These rules match the Tokio team's official guidance and Clippy lint `await_holding_lock`.

### 5. `tokio::select!` & Control Flow

`select!` is powerful and dangerous. Use it with explicit invariants.

- **Cancellation safety first.** Every branch future in `select!` must be safe to drop at any await point. If a future is not cancellation-safe (e.g., `AsyncWrite::write`), complete it in a dedicated task or use a channel.
- **Prefer `biased` only when order is load-bearing.** If `biased` is used, document the ordering invariant in a comment. Default to fair scheduling unless starvation is proven.
- **Always include the cancellation token in loops.** A `loop { select! { ... } }` that does not select on `token.cancelled()` is not cancellable.
- **Never `select!` on an unbounded stream and a timer without backpressure.** The stream will starve the timer.

**Rationale:** `select!` drops uncompleted futures. Non-cancellation-safe futures lose data or corrupt state. Alice Ryhl's "Async Cancellation" and the Tokio documentation treat this as the #1 source of async bugs in production Rust.

### 6. Async I/O & Blocking Boundaries

The executor thread must never block.

- **`tokio::fs` for all filesystem I/O in async context.** Never `std::fs` in an async task.
- **`spawn_blocking` for CPU-heavy work.** Compression, hashing, large JSON parsing, regex compilation, and tree-sitter parsing must run in `spawn_blocking` or a dedicated thread pool.
- **Timeouts on all external operations.** Every `Command`, network read, and human-facing prompt must have a `tokio::time::timeout` with a documented duration (Rust Safety Rule #6).
- **Timeout rationale is part of the contract.** Do not use magic numbers. Either define a `const TIMEOUT_SECS: u64` with a comment, or accept timeout as a config parameter.

**Rationale:** Blocking the async executor freezes all tasks on that thread. `spawn_blocking` isolates CPU work. This is the standard across every async Rust codebase from AWS to Discord.

### 7. State Sharing & `Send`/`Sync`

Types that cross task boundaries must be thread-safe.

- **All types sent to `spawn` or across `.await` must be `Send`.** Compile-time enforcement is not enough — review `Rc`, `RefCell`, and raw pointers in structs.
- **`!Send` types must stay local.** If a type is `!Send` (e.g., a UI handle), keep it in `tokio::task::LocalSet` or a dedicated thread. Do not sneak it across `spawn`.
- **Prefer `Arc<str>` and `Arc<[T]>` over `String`/`Vec` in shared read-only state.** Avoid cloning large strings on every access.
- **SQLite connections:** use a single writer task with a channel (actor pattern) or `tokio::sync::Mutex<Connection>` with WAL mode and a busy timeout. Concurrent writers on the same connection without coordination are banned.

**Rationale:** `Send`/`Sync` bugs are compile-time errors in safe Rust, but `Rc` inside a struct that looks `Send` is a runtime footgun. SQLite is single-writer by design; pretending otherwise produces "database is locked" or silent corruption.

### Shutdown Sequence Reference

For any subsystem with async tasks (Wire worker, team runtime, goal engine), the shutdown order must be:

```
1. Stop accepting new work (close listen socket / cancel root token).
2. Signal children to stop (cancel child tokens).
3. Drain bounded channels (flush remaining events to sink).
4. Join all tasks with a timeout (e.g., 30 s).
5. Force abort any remaining tasks.
6. Emit final state / proof / metric flush.
7. Exit.
```

Every subsystem must document its own steps 1–7 in a module-level comment or README.

### Migration Path

Not all async code complies yet. When editing a module with async tasks:
- Replace bare `tokio::spawn` with `JoinSet` or stored `JoinHandle` before adding features.
- Add `CancellationToken` to any new long-running loop.
- Document bounded channel capacities when adding or modifying channels.
- Do not add new fire-and-forget spawns. Do not retrofit the entire codebase in one PR.

## Error Handling Doctrine (Hard Constraints)

> Cross-references: [Rust Safety Rules](#rust-safety-rules-hard-constraints) (panic/unwrap ban), [Async Architecture](#async--concurrency-architecture-hard-constraints) §2 (shutdown error handling), [Observability Standards](#observability-standards-hard-constraints) §3 (structured error fields), [Security](#security--trust-boundaries-hard-constraints) §6 (secrets in errors).

Errors are a public API. A user, a Wire client, and an operator must each receive the right error representation at the right boundary. Sloppy error handling breaks CLI contracts, leaks secrets into JSON streams, and turns graceful shutdown into data loss.

### Meta Principle

**Every error has an owner, a representation, and a consumer. Never swallow an error. Never show a stack trace to a user. Never send human prose down a machine channel.**

### 1. `anyhow` vs `thiserror` — The Boundary Rule

The choice of error type is architectural, not stylistic.

- **Library code under `src/` (excluding `src/cli/` glue) uses `thiserror`.**
  - Every public function that can fail returns a specific `enum` error type with `#[derive(Error)]`.
  - Callers must be able to `match` on variants to decide retry, abort, or fallback.
  - Example: `WireClient::connect` returns `Result<T, WireError>` where `WireError::Timeout` and `WireError::ProtocolMismatch` are distinct variants.

- **Application glue (`src/cli/`, `src/main.rs`, orchestration layer) uses `anyhow`.**
  - `.context("...")?` is the standard pattern at CLI boundaries.
  - `anyhow` is allowed only where the error is terminal (reported to user or logged) and will not be matched by a caller.

- **Wire protocol uses JSON-RPC error codes.**
  - Human text in `message` fields is banned (Project Contract #6).
  - Errors carry a machine-readable `code` and optional structured `data`.

- **Never mix `anyhow` in public library API.**
  - If a function is `pub` and returns `anyhow::Result`, it must be refactored to a typed `thiserror` enum.

**Rationale:** `thiserror` at library boundaries gives callers control. `anyhow` at application boundaries gives operators context. Mixing them creates opaque APIs that cannot be tested or recovered. This is the standard in the Rust ecosystem (Axum, Tower, Tokio all use typed errors at their boundaries).

### 2. Exit Codes (CLI Contract)

`omk` is a CLI tool. Exit codes are part of the public contract and must be stable across releases.

| Code | Meaning | When |
|---|---|---|
| `0` | Success | Normal completion, proof ready |
| `1` | General failure | Validation failed, gate failed, I/O error, Wire error |
| `2` | Misuse | Invalid arguments, unknown subcommand (Clap) |
| `130` | Interrupted | `SIGINT` (`Ctrl+C`) or `CancellationToken` triggered by user |
| `>128` | Fatal signal | `128 + signal_number` for unhandled signals |

Rules:
- **Never use magic numbers.** Define exit codes as a named `const` or an `enum` in `src/cli/`.
- **Child process exit codes are not propagated blindly.** A gate that exits `1` does not make `omk` exit `1` unless the gate is required and the proof status is `Failed`.
- **Distinguish "error" from "failure."** An *error* is a bug or unexpected condition (exit `1`). A *failure* is a validated negative result (e.g., gate `test` found bugs) that produces a proof artifact and may still exit `0` if the proof system captured it correctly.

**Rationale:** Scripts and CI pipelines depend on stable exit codes. Changing `exit(1)` semantics between releases breaks automation.

### 3. Error Channels — Where Errors Go

An error must be routed to exactly one primary consumer. Dual reporting creates spam and secret leakage.

- **User-facing (human):** `stderr`, terminal, TUI notification.
  - Use `Display` (`{}`), never `Debug` (`{:?}`).
  - No stack traces, no internal types, no module paths.
  - Must be actionable: `"Failed to read config at ~/.config/omk/config.toml: permission denied"`.

- **Machine-facing (Wire / JSONL / proof):** structured data.
  - Use JSON-RPC error objects, event records, or proof `failures` array.
  - No human prose in machine channels (Project Contract #6).
  - Redact secrets before serialization (Project Contract #12).

- **Operator-facing (logs / tracing):** structured spans and fields.
  - Full error chain, file paths, `run_id`, `worker_id`.
  - Use `tracing::error!(error = %e, "...")` with structured fields.
  - This is the only channel that may contain stack traces (`#[cfg(debug_assertions)]` or `RUST_BACKTRACE=1`).

- **Proof artifacts:** `ProofStatus::Failed` with structured `failures`.
  - A gate failure is not an application error; it is a validated result that belongs in the proof.

**Rationale:** Project Contract #6 exists because mixing human and machine output breaks parsers, dashboards, and replay systems. Three separate channels with clear ownership prevent leakage.

### 4. Error Context & Chaining

Every boundary crossing must add context. Every wrapper must add meaning.

- **Add `.context("...")` at I/O, network, and process boundaries.**
  - Good: `fs::read(path).await.context("failed to read config")?`
  - Bad: `fs::read(path).await?` (loses path information).

- **Do not wrap without adding context.**
  - Bad: `inner_call().map_err(|e| MyError::Inner(e))?` (opaque translation).
  - Good: `inner_call().context("building task graph for worker {id}")?`.

- **`thiserror` messages must be lowercase without trailing punctuation.**
  - Good: `#[error("connection refused")]`
  - Bad: `#[error("Connection refused.")]`

- **Preserve the cause chain.**
  - `#[source]` or `#[from]` must be used so that operators can trace the full chain in logs.

**Rationale:** Context is the difference between `"operation failed"` and `"failed to spawn gate 'clippy' for worker-3: executable not found in PATH"`. The first is useless for debugging; the second is actionable.

### 5. Silent Errors Are Banned

An error that is not observed, logged, or propagated does not exist. It is a silent corruption.

- **`let _ = ...` on `Result` is banned unless explicitly justified.**
  - Bad: `let _ = sender.send(event).await;`
  - Good: `sender.send(event).await.map_err(|e| tracing::warn!("event sink lagging: {e}"))?;`

- **Explicit ignore requires a comment and `ALLOW` annotation.**
  ```rust
  // Safe to ignore: receiver is gone because the subsystem shut down;
  // the event is being discarded as part of graceful degradation.
  #[allow(unused_must_use)]
  let _ = sender.try_send(event);
  ```

- **Timeouts must be named and surfaced.**
  - Bad: `let _ = tokio::time::timeout(Duration::from_secs(10), work).await;`
  - Good: match the timeout, log the duration, and convert to a typed error.

- **Async Architecture connection:** Channel send errors during shutdown are not "silent drops". They must be logged at `warn` level and emitted as a `WorkerDroppedEvent` if the proof system cares about loss.

**Rationale:** Silent errors are the leading cause of "impossible" bugs in async Rust. The compiler warns with `unused_must_use` for a reason. Ignore it only with a written justification.

### 6. Error Paths in Async

Async Architecture requires graceful shutdown. Error handling must support this, not fight it.

- **Channel send failure = receiver gone.**
  - Log at `warn`.
  - Treat as cancellation signal: stop producing, begin local cleanup.
  - Do not panic. Do not retry indefinitely.

- **Spawned task panic = `JoinError`.**
  - The parent must `await` the `JoinHandle` and handle `Err(JoinError)`.
  - Log at `error` with task name and span context.
  - Update proof status to `Failed` if the task was critical.

- **Timeout errors must distinguish "slow" from "dead."**
  - `OperationTimeout` = the operation may have partially completed. Do not assume idempotency unless documented.
  - `OperationAborted` = explicitly cancelled via `CancellationToken`. Safe to assume cleanup is in progress.

- **Shutdown sequence errors (Async Architecture step 4–6).**
  - If a task fails to join within the timeout: log, abort, and continue shutdown.
  - If flush fails: log the error, but do not halt the remaining shutdown steps.
  - The final exit code reflects the original reason for shutdown, not a secondary flush error.

**Rationale:** In async systems, errors cascade. A channel closure can trigger a timeout, which triggers a cancel, which triggers a flush error. Handling each stage explicitly prevents cascading panic or deadlock.

### 7. Panic vs Result — Expansion from Safety Rules

Rust Safety Rules ban `unwrap`, `expect`, and `panic!`. This section adds nuance for edge cases.

- **`unreachable!()` is allowed only with a proof comment.**
  ```rust
  // Invariant: `policy` is validated at parse time; all variants are handled above.
  #[allow(unreachable_code)]
  unreachable!("unhandled approval policy: {:?}", policy);
  ```

- **`todo!()` is banned in committed code.**
  - It compiles but panics at runtime. Use `unimplemented!("reason")` only in draft branches, never in `main`.

- **Invariant violations that indicate programmer error may use `panic!` only in `debug_assert!` shape.**
  - If the invariant can be recovered, return `Result`.
  - If the invariant indicates corruption (e.g., SQLite foreign key violated internally), log a critical error and abort the operation, not the process.

**Rationale:** Panics in async Rust poison tasks and may leave locks, channels, and child processes in an undefined state. Prefer graceful degradation.

### 8. Testing Error Paths

An error path that is not tested is not handled. It is hope.

- **Unit tests must cover every `Error` variant at least once.**
  - If `WireError` has 4 variants, there must be 4 unit tests (or a parameterized test) exercising each.

- **Property tests (SOTA Testing Tier 2) must include invalid inputs.**
  - Malformed JSON, oversized payloads, invalid UTF-8, path traversal strings.

- **Fault injection (SOTA Testing Tier 6) must verify recovery, not just failure.**
  - After a timeout, is the channel drained?
  - After a process kill, is the temp directory cleaned?
  - After a cancellation, is the proof artifact written?

- **CLI error tests must assert exit codes.**
  - `assert_cmd` tests should verify both `output.status.code()` and the content of `stderr`.

**Rationale:** Error paths are where production systems die. They deserve the same testing rigor as success paths.

### Migration Path

Not all code uses typed errors yet. When editing a module:
- Refactor `anyhow::Result` in `pub` functions to `thiserror` enums.
- Replace silent `let _ = ...` with explicit error handling or a documented `#[allow]`.
- Add snapshot tests (SOTA Testing Tier 3) when changing CLI output.
- Migrate module-by-module; do not refactor untouched code.

## Data Layer & State Management (Hard Constraints)

> Cross-references: [Async Architecture](#async--concurrency-architecture-hard-constraints) §2 (shutdown flush), §7 (SQLite connection strategy), [Security](#security--trust-boundaries-hard-constraints) §3 (worktree boundaries), [SOTA Testing](#sota-testing) Tier 2 (roundtrip property tests).

OMK persists events, proof artifacts, goal state, and configuration. On-disk state is a public contract: users upgrade, downgrade, and inspect files directly. Schema changes must not corrupt existing data.

### Meta Principle

**The database and event log are append-only contracts. Destructive migrations are banned. Backward compatibility is not a feature — it is a requirement.**

### 1. SQLite & Migrations

- **Additive-only schema changes.** New columns are `NULL`able or have sensible defaults. Dropping columns, tables, or renaming is banned; create a new table and migrate data if necessary.
- **Migration versioning.** Use `PRAGMA user_version` or an explicit `schema_migrations` table. Each migration is a numbered SQL script with a checksum.
- **Backward compatibility horizon.** On-disk format must be readable by at least the previous minor version (semver). If a breaking change is unavoidable, bump the minor version and document the migration path.
- **Transaction boundaries.** One logical operation = one explicit transaction. Do not rely on auto-commit for multi-step writes. Use `BEGIN IMMEDIATE` when contending with concurrent writers.
- **WAL mode.** SQLite must use WAL (`PRAGMA journal_mode=WAL`) for read concurrency and crash resilience. Checkpointing strategy (`TRUNCATE` vs `PASSIVE`) must be documented per subsystem.

### 2. Connection Strategy

- **Single writer or actor pattern.** Either one dedicated task owns the connection and receives writes via a channel, or use `tokio::sync::Mutex<Connection>` with a busy timeout. Un-coordinated concurrent writers on the same connection are banned (see Async Architecture §7).
- **Connection lifetime.** The connection is opened at subsystem startup and closed during graceful shutdown step 6 (Async Architecture). Do not open/close per query.
- **Busy timeout.** Set `busy_timeout` to at least 5000 ms to handle transient lock contention, not zero.

### 3. Event Log & Proof Artifacts

- **Append-only.** `events.jsonl` and proof JSON files are append-only or versioned. Overwriting historical records is banned.
- **Rotation, not mutation.** When files grow large, rotate them (timestamped archive). Do not truncate or rewrite in place.
- **Structured format stability.** The JSON schema for events and proof must carry a version field. Changes require a compatibility alias or migration note (Project Contract Rule #4).

### 4. State Isolation

- **Worktree-scoped state.** State files (events, proof, temp data) live inside the worktree or a designated XDG state directory. Writing outside the worktree boundary requires explicit user approval (Security §3).
- **No secrets in state files.** If state must reference a secret, store a key ID or path to a restricted file, not the secret itself.

### 5. Testing Data Contracts

- **Migration tests.** Every migration script must have a test that applies it to a known schema and verifies the result.
- **Roundtrip tests.** Serialize → persist → read → deserialize must be lossless for all state types (events, proof, config).
- **Property tests (SOTA Testing Tier 2).** Generate arbitrary event sequences and verify that append-only log parsing never panics.

### Migration Path

Not all modules use WAL or explicit transactions yet. When editing a module with database or file state:
- Add WAL mode and busy timeout before relying on concurrent reads.
- Wrap multi-step writes in explicit transactions.
- Add roundtrip tests for any new state type.
- Do not introduce destructive schema changes in patch releases.

## Security & Trust Boundaries (Hard Constraints)

> Cross-references: [Async Architecture](#async--concurrency-architecture-hard-constraints) §6 (spawn_blocking, process sandboxing), [Error Handling Doctrine](#error-handling-doctrine-hard-constraints) §3 (error channels), [Observability Standards](#observability-standards-hard-constraints) §5 (secrets in spans), [Data Layer](#data-layer--state-management-hard-constraints) §4 (state isolation).

OMK is an autonomous runtime that executes LLM-generated instructions, spawns child processes, and manipulates user repositories. The LLM output is **untrusted by default**. Every layer below the Wire protocol must validate, restrict, and audit everything it receives.

### Meta Principle

**Defense in depth. The LLM is a user-supplied input source, not a trusted system component. No single bug at any layer must allow privilege escalation, data exfiltration, or unauthorized destructive operations.**

### 1. Trust Model & Approval Layers

Trust is not binary. It is a spectrum enforced by code, not by documentation.

- **LLM output is untrusted.** Every tool call, file path, and command string arriving through the Wire protocol is potentially adversarial until validated.
- **Automatic execution is restricted.** Operations that are read-only and scoped to the worktree may proceed automatically (subject to policy). Any destructive, network, or out-of-scope operation requires explicit approval or human-in-the-loop.
- **Approval policy is enforced at the runtime layer, not the Wire client.** Even `ApprovalPolicy::Yolo` does not bypass file system or process sandboxing. Yolo means "skip human prompt", not "skip all checks".
- **System commands are not LLM suggestions.** The runtime must distinguish between internal control commands (cancellation, shutdown, config) and LLM tool calls. Internal commands must not be injectable through the Wire message stream.
- **Prompt injection defense:** System-level instructions (approval proxy, policy engine) must not be overridable by user or LLM messages. Wire protocol fields carrying policy must be signed or set out-of-band.

**Rationale:** OWASP LLM Top 10 (LLM01 Prompt Injection, LLM02 Insecure Output Handling) treats LLM output as attacker-controlled input. Positioning the LLM as a trusted component is a category error.

### 2. Input Validation

All external input must be validated at the boundary before it reaches business logic.

- **Path canonicalization is mandatory.** Every path from a tool call must be passed through `std::fs::canonicalize` or equivalent and verified against an allowlist.
- **Path traversal is rejected at the Wire boundary.** `../`, absolute paths, null bytes, and symlink escapes are banned. Rejection happens before the path reaches `tokio::fs`.
- **Command arguments are arrays, not strings.** Gates and tool calls must use `Command::arg` / `argv` arrays. Shell string interpolation (`sh -c "..."`) with untrusted input is banned.
- **Schema-first deserialization.** Wire JSON messages must be validated against a JSON Schema or strict struct deserialization with `deny_unknown_fields` where appropriate, before conversion to domain types.
- **Size limits.** Incoming Wire payloads, file reads, and event buffers must have enforced size limits. Unbounded reads are banned.

**Rationale:** Canonicalization failure is the root cause of most path traversal CVEs (CVE-2023-28432, etc.). Array arguments prevent shell injection by construction.

### 3. File System Boundaries

The worktree is the primary security boundary. File operations must not escape it.

- **Read scope:** By default, `read_file` may only read inside the project worktree. Reading outside the worktree requires explicit user approval, regardless of path canonicalization.
- **Write scope:** Writes are restricted to the worktree and designated temp directories (`tempfile::TempDir`). Writing to `~/.ssh`, `/etc`, system directories, or other repositories is banned.
- **Dotfile policy:** Reading or writing hidden files (`.env`, `.gitconfig`, `.ssh/*`) inside the worktree requires explicit approval. The worktree boundary does not grant automatic access to secrets.
- **Symlink hardening:** After canonicalization, verify the resolved path is still within the worktree. Symlinks pointing outside are treated as out-of-scope and rejected.
- **No deletion without audit.** `remove_file`, `remove_dir`, `git reset --hard` must emit a security event before execution and require approval unless scoped to a temp directory.

**Rationale:** The worktree is the unit of trust. If `omk` can write to `~/.bashrc` because the project happens to contain a symlink, the worktree boundary is meaningless.

### 4. Process Execution & Sandboxing

Gates and child processes run user code. They must be contained.

- **Least privilege.** Gate commands execute with the user's UID. No `sudo`, no setuid, no privilege escalation. If a gate requires elevated permissions, it fails with a typed error and requires human execution outside OMK.
- **Controlled `PATH`.** The runtime must sanitize or override `PATH` for gate execution. Inheriting the user's full `PATH` (which may include `.`, `./node_modules/.bin`, or attacker-controlled directories) is banned.
- **Working directory lock.** Child processes must start in the worktree or a designated temp dir. Changing to `/` or user home is not permitted without explicit approval.
- **Resource limits.** Gates must have:
  - Timeout (Rust Safety Rule #6).
  - Optional memory limit where the OS supports it (`ulimit`, cgroups).
  - Process group kill: on timeout or cancellation, kill the entire process group, not just the parent, to prevent orphan grandchildren.
- **No execution of generated code without inspection.** If an LLM generates a shell script or binary as part of a task, it must be written to disk and treated as untrusted until reviewed. Auto-execution of generated scripts is banned.
- **`kill_on_drop(true)` is mandatory** (Rust Safety Rule #7), but for security-critical child processes also set `process_group(0)` or platform equivalent to ensure tree-wide termination.

**Rationale:** Gates are arbitrary code execution by design. The only thing separating "helpful automation" from "remote code execution" is the sandbox around the gate.

### 5. Network Boundaries

The runtime must not become a proxy for arbitrary LLM-driven network attacks.

- **Allowlist for outbound connections.** The Wire client and MCP client may only connect to explicitly configured endpoints. Arbitrary URLs from LLM tool calls are rejected.
- **MCP server allowlist.** Only MCP servers declared in configuration may be invoked. Dynamic MCP server registration via LLM output is banned.
- **No network in gates unless declared.** A gate running `cargo test` should not make network requests. If it does, the network policy must be explicitly documented for that gate.
- **Localhost protection.** Connections to `localhost`, `127.0.0.1`, or private IPs from LLM-driven network calls require explicit approval to prevent SSRF against local services (databases, Docker, admin interfaces).

**Rationale:** SSRF (Server-Side Request Forgery) via LLM tool calls is a known attack vector. Restricting outbound traffic to an allowlist prevents data exfiltration and lateral movement.

### 6. Secret Hygiene

Secrets must be invisible to the LLM, to the logs, and to the proof artifacts.

- **Redaction is mandatory at boundaries.** Project Contract #12 is expanded here: logs, events, proof JSON, Wire messages, error messages, tracing spans, and debug dumps must pass through the centralized redaction filter.
- **No secrets in CLI arguments.** API keys, tokens, and passwords must never be passed as CLI arguments (visible in `ps`). Use environment variables or temporary files with restricted permissions (`0600`).
- **No secrets in generated code.** The LLM must not be given access to `.env` files, `~/.netrc`, or SSH keys. If the LLM needs an API key for a subtask, the runtime injects it via env at spawn time, not via prompt text.
- **Memory hygiene:** Secret buffers (`String`, `Vec<u8>`) must not be cloned unnecessarily. Use zeroing-on-drop where practical (`secrecy` crate or equivalent) for high-value tokens.

**Rationale:** A compromised LLM session or a leaked proof artifact must not contain credentials. Redaction at the boundary is the only way to enforce this invariant globally.

### 7. Audit & Evidence

Security-relevant actions must leave tamper-evident evidence.

- **Every security action is an event.** File read/write, command execution, network connection, approval decision, policy change, and authentication attempt must emit an `Event` record.
- **Event structure:** `timestamp`, `actor` (`llm` / `human` / `system`), `action`, `target` (canonicalized path or command), `policy` applied, `result` (`allowed` / `denied` / `error`).
- **Proof inclusion:** The proof artifact must contain a summary of security events (approvals granted, files modified, commands executed) for post-hoc audit.
- **Append-only event log.** The local event sink (`events.jsonl`) must be append-only. Rotation is allowed; mutation or deletion of historical events is banned.

**Rationale:** If something goes wrong, the operator must be able to reconstruct "what the LLM was allowed to do and when" without relying on the LLM's own memory or human recollection.

### 8. Dependency Supply Chain

A vulnerability in a dependency is a vulnerability in OMK.

- **cargo-deny is mandatory.** The existing CI check for advisories, licenses, and sources is a hard gate. PRs that introduce dependencies with open CVEs in the advisory database do not merge.
- **New dependency security review.** Before adding a crate, check:
  - Open CVEs in `cargo audit` / RustSec.
  - Maintenance status (commits in last 12 months, responsive maintainers).
  - Transitive dependency count.
- **Cargo.lock is sacred.** Do not ignore `Cargo.lock` in applications. Reproducible builds are a security property.
- **Feature minimalism.** Enable only the required feature flags. Default features must be audited before inclusion.

**Rationale:** Supply chain attacks (xz, log4j) demonstrate that dependencies are an extension of the attack surface. `cargo-deny` is the baseline, not the ceiling.

### Migration Path

Not all modules validate input or sandbox processes yet. When editing a module:
- Add path canonicalization to any new file operation.
- Use `Command::arg` arrays; never introduce shell interpolation.
- Add security event emission for new destructive operations (file write, delete, process spawn).
- Do not broaden scope without explicit approval logic.

## Observability Standards (Hard Constraints)

> Cross-references: [Error Handling Doctrine](#error-handling-doctrine-hard-constraints) §3 (operator-facing logs), [Async Architecture](#async--concurrency-architecture-hard-constraints) §1 (span ownership), [Security](#security--trust-boundaries-hard-constraints) §7 (audit events).

OMK is an async runtime with concurrent workers, Wire protocol sessions, gate execution, and a TUI. Without consistent observability, production debugging becomes archaeology. Every agent editing the codebase must emit, structure, and consume telemetry the same way.

### Meta Principle

**Observability is a public API. Spans, fields, and metric names are contracts. Changing them breaks dashboards, alerts, and operator playbooks.**

### 1. Span Naming & Structure

Spans are the primary unit of context in async code. They must be predictable and parseable.

- **Naming convention:** `snake_case`, module-path style.
  - Good: `omk::wire::client::connect`, `omk::runtime::goal::execute`, `omk::gates::run`
  - Bad: `connect`, `do_stuff`, `handleRequest`
- **Instrument public async boundaries.** Every `pub async fn` that crosses an I/O or module boundary must use `#[instrument]` or `tracing::info_span!`.
- **Parent-child relationships must reflect the call graph.** A Wire worker span is a child of the run span. A gate span is a child of the worker span.
- **Skip large or sensitive arguments.** Use `#[instrument(skip(payload, secrets, api_key))]`. If a field is sensitive, skip it; do not redact inline in the format string.

**Rationale:** Inconsistent span names break tracing aggregation (Jaeger, Tempo, Honeycomb). Module-path naming is the standard in Tokio, Axum, and Tower.

### 2. Level Policy

Levels are not personal preference. They are a contract with the operator.

| Level | Meaning | Examples |
|---|---|---|
| `ERROR` | Correctness impact or operator intervention required | Gate crash, Wire disconnect, DB corruption, flush failure, permission denied on critical path |
| `WARN` | Degraded but recovered, or anomaly detected | Timeout retry succeeded, channel backpressure, stale heartbeat, approval fallback triggered |
| `INFO` | Business event that an operator cares about | Run started/completed, worker spawned/stopped, gate passed/failed, goal claimed |
| `DEBUG` | Internal state transition for developer debugging | Approval decision details, event batch flush, wire message parsed, SQL query plan |
| `TRACE` | Protocol bytes, TUI frames, per-line I/O | Wire JSON payload, terminal buffer diff, tree-sitter parse tree |

Rules:
- **`INFO` is not a printf for debugging.** `tracing::info!("here!!!")` is banned. If you need a tracepoint while developing, use `DEBUG` or `TRACE` and remove it before merge.
- **`WARN` requires a recoverable outcome.** If the code returns `Err` to the caller, it is at least `WARN` (or `ERROR` if unrecoverable).
- **`ERROR` must be actionable.** Every `ERROR` log should tell the operator what failed and what the system did about it (retried, aborted, degraded).

**Rationale:** In production, operators set `RUST_LOG=omk=info`. If `INFO` is noisy, they miss real events. If `WARN` is underused, they miss degradation signals.

### 3. Structured Fields & Correlation

Free-text log messages are for humans reading one line. Fields are for machines aggregating millions.

- **Use structured fields, not format interpolation.**
  - Good: `tracing::info!(run_id = %run_id, worker_id = %worker_id, "worker spawned");`
  - Bad: `tracing::info!("worker {} spawned for run {}", worker_id, run_id);`

- **Mandatory fields per domain:**
  - **Run scope:** `run_id`
  - **Worker scope:** `run_id`, `worker_id`, `role`
  - **Wire scope:** `request_id` or `method`, `wire_version`
  - **Gate scope:** `gate_name`, `command_line` (sanitized, no secrets), `exit_code`
  - **Approval scope:** `policy`, `action`, `sender`

- **Dynamic field keys are banned.** `tracing::info!(%dynamic_key = value)` breaks structured parsing. Keys must be static string literals.
- **Correlation ID propagation.** When crossing an async boundary (spawn, channel send), the `run_id` and `trace_id` must be propagated via the context or explicit message fields.

**Rationale:** Structured fields enable `grep`-free querying (`trace_id = 'abc123'`), correlation across distributed spans, and automated alerting. Dynamic keys make parsing impossible.

### 4. Metrics

Events and spans are the source of truth. Metrics are derived views.

- **Naming convention (Prometheus-style):**
  - Counters: `omk_<entity>_<action>_total`
    - `omk_events_emitted_total`
    - `omk_gates_executed_total`
    - `omk_wire_messages_sent_total`
  - Histograms/gauges: `omk_<entity>_<metric>_<unit>`
    - `omk_wire_latency_ms`
    - `omk_gate_duration_ms`
    - `omk_proof_size_bytes`

- **Labels (dimensions):**
  - `run_id` (high cardinality — use sparingly, prefer aggregation)
  - `worker_role` (`coder`, `architect`, etc.)
  - `gate_name` (`fmt`, `clippy`, `test`)
  - `status` (`passed`, `failed`, `timeout`)
  - `policy` (`yolo`, `human`, `never`)

- **Metric names are append-only.** Renaming a metric is a breaking change for dashboards. Deprecate with a `_deprecated` suffix and introduce a new name.
- **Derive from events where possible.** The `EventSink` is the canonical source of business events. Metrics should be aggregated from events, not duplicated as ad-hoc increments in business logic.

**Rationale:** Metrics are a public API for operators. Ad-hoc naming creates metric sprawl. Deriving from events ensures consistency between the event log and the dashboard.

### 5. Secrets & PII in Observability

Tracing is a boundary. Secrets must not leak through it.

- **Never put secrets in span fields or log messages.** This includes tokens, API keys, passwords, private keys, and session cookies.
- **Use `skip()` in `#[instrument]`.** If a function receives a secret argument, it must be skipped from the span.
- **Redact before structured serialization.** If a field might contain a path under `$HOME`, a git URL with embedded credentials, or an env var value, pass it through the same redaction pipeline used for events (Project Contract #12, Security §6).
- **No file contents in TRACE/DEBUG unless gated.** Logging the contents of `~/.ssh/id_ed25519` or `.env` at `TRACE` level is equivalent to a breach. If file contents must be logged for debugging, use a feature flag or a separate, access-controlled debug dump mechanism.

**Rationale:** Tracing backends (files, journald, APM vendors) often have weaker access controls than secret managers. A span is a leak surface.

### 6. Human vs Machine vs Operator Output

OMK has three distinct consumers of output. They must not be mixed.

| Consumer | Channel | Content | Format |
|---|---|---|---|
| User (human) | `stderr`, TUI | Actionable messages, progress, errors | Human prose, localized if applicable |
| Machine (Wire / JSONL / proof) | `stdout`, files | Structured data, events, proof artifacts | JSON, JSONL, Markdown |
| Operator (engineer / SRE) | tracing subscriber (file/journald/OTLP) | Diagnostics, context, correlation | Structured spans + fields |

- **Library code (`src/` outside `cli/`) never writes to stdout or stderr.** It emits events, returns `Result`, or records spans. Writing to stdout/stderr from library code is banned because it corrupts machine-readable output (Project Contract #6).
- **CLI code (`src/cli/`) owns the user channel.** It translates `Result` and events into human-readable `stderr` messages.
- **TUI owns the real-time user view.** It subscribes to events, not logs. Do not write TUI updates via `println!`.

**Rationale:** Mixing channels is the #1 cause of broken JSON parsers, corrupted proof artifacts, and confused operators.

### 7. Performance & Sampling

Observability must not observably slow down the system.

- **No heavy computation in span fields.** `serde_json::to_string` in a `tracing::info!` call is banned. Pre-compute or skip.
- **TRACE is free when disabled.** Use `tracing::trace!` liberally for deep debugging, but ensure it compiles to a no-op when the subscriber is not subscribed at that level.
- **Production default: `INFO`.** `DEBUG` is enabled per-module for targeted investigation (`RUST_LOG=omk::wire=debug,omk::runtime=info`). `TRACE` is never enabled globally in production.
- **Do not instrument tight loops.** A span has overhead. Instrument operations that cross I/O, module, or task boundaries. For hot loops, use a counter or a single span around the loop, not per-iteration spans.

**Rationale:** `tracing` is zero-cost for disabled levels, but span creation and field evaluation are not. In a TUI or Wire loop, excessive spans can add milliseconds per frame.

### 8. Testing & Verification

Observability contracts must be tested like any other contract.

- **Unit tests may assert on spans.** Use `tracing-test` or manual `MockSubscriber` to verify that a specific span was entered with expected fields.
- **Security-critical paths must leave trace evidence.** Policy rejections, path traversal blocks, and approval denials must emit a `WARN` or `INFO` span that can be asserted in tests.
- **No stdout/stderr noise in tests.** Library tests that print to stdout/stderr fail the "clean output" standard. Use events or tracing.
- **Fault injection tests (SOTA Testing Tier 6) should assert recovery spans.** After a simulated network drop, assert that `wire_client_reconnect` span was emitted.

**Rationale:** If an approval denial does not leave a span, the operator cannot audit it. If a test does not assert the span, the observability contract is not verified.

### Migration Path

Not all modules emit structured spans yet. When editing a module:
- Add `#[instrument]` to new or modified `pub async fn` boundaries.
- Replace format-string logs with structured fields.
- Skip secrets from spans; do not add new logging of sensitive data.
- Update module README if new metrics or events are introduced.

## SOTA Testing

SOTA (State of the Art) testing is not "100 % line coverage". It is a **system of guarantees** where every semantic change that breaks a contract is caught automatically within a single PR iteration. For oh-my-kimi — an async runtime with a Wire protocol, proof system, and CLI surface — this means seven ordered tiers, each catching bugs that lower tiers cannot.

### Meta Principle

**Each tier catches a bug class that is structurally invisible to the tiers above it.**

- Unit tests do not find races.
- Property tests do not verify visual output.
- Snapshots do not find deadlocks.
- DST does not check business logic invariants.

Only the stack together gives confidence, not the sum of individual tests.

### Tier 0 — Type-Level Contracts

Prefer expressing invariants in types before tests or comments.

- Use newtypes and parser constructors that make invalid states unrepresentable.
- Use `debug_assert!` only where the type system is insufficient.
- **Rationale:** "Make illegal states unrepresentable" is cheaper and stronger than any test. This is the standard across Tokio, Rust-for-Linux, and the Rust std library itself.

### Tier 1 — Unit Tests (In-Memory, Trait-First)

Unit tests live in `#[cfg(test)] mod tests` inside the source file. They must test **one function or method** through its public interface.

Rules:
- Use only in-memory mocks (`MockWireClient`, `MockEventSink`, `MockCostSink`).
- No `tokio::fs::write`, no temp files, no shell scripts, no real network calls in unit tests.
- Test names describe the **invariant**, not "does it work":
  - Good: `rejects_negative_timeout`, `roundtrip_preserves_order`
  - Bad: `test_foo_works`, `test_bar_ok`
- `unwrap()` is allowed for brevity, but prefer `?` when it keeps the test readable.
- No `std::thread::sleep` — use `tokio::time::pause` + `advance`.
- **Rationale:** Tokio, Axum, and Bevy all use trait mocks as the primary unit-testing strategy. It allows thousands of tests to run in seconds with zero side effects.

### Tier 2 — Property-Based Tests (Parsing / Serde / Redaction)

Any parser, serializer, or redaction logic must have property tests.

- Use `proptest` (or equivalent) for:
  - serde roundtrip: `serialize ∘ deserialize == id`
  - Wire protocol codec roundtrip
  - Redaction: generated strings containing secret-like patterns must be fully scrubbed
- **Rationale:** Hand-written examples cover <0.1 % of the input space. AWS Firecracker, Cloudflare Quiche, and Discord's Rust services use property-based testing as the standard for protocol boundaries.

### Tier 3 — Snapshot Tests (Output Contracts)

Any human-readable output that is part of the public API must be snapshot-tested.

- CLI `--help`, `--version`, error messages.
- Proof Markdown output.
- TUI render output.
- Use `insta` (not custom snapshot helpers).
- Redact unstable fields (timestamps, UUIDs, temp paths) with `insta::with_settings!`.
- Update snapshots via `cargo insta review`, never by hand.
- **Rationale:** `insta` is the de-facto standard in Rust (used in rustc, cargo, ripgrep, alacritty). Snapshots are the only automated way to detect accidental output changes that break user scripts or documentation.

### Tier 4 — Integration Tests (Black-Box)

Integration tests live in `tests/`. They exercise the library or binary from the outside.

- Use `assert_cmd` + `predicates` for CLI assertions.
- Use `omk::test_helpers::isolated_xdg_env()` for filesystem isolation.
- No real network calls in CI — use mocks, fakes, or local fake servers.
- **Rationale:** Integration tests verify that modules that pass in isolation also compose correctly. The `tests/` directory is a first-class citizen in the Rust ecosystem.

### Tier 5 — Deterministic Simulation Testing (Concurrency)

Any code involving `Mutex`, `RwLock`, `mpsc`, `tokio::select!`, or shared mutable state must have DST coverage.

- Use `shuttle` (async scheduling) or `turmoil` (network simulation) to exhaustively explore execution schedules.
- Target: approval proxy races, goal supervisor claims, event delivery ordering, channel backpressure.
- **Rationale:** AWS (S3, DynamoDB), FoundationDB, and TigerBeetle treat DST as the primary way to test concurrent systems. "It doesn't reproduce locally" is not an acceptable concurrency-testing strategy.

### Tier 6 — Fault Injection (Chaos in Tests)

Runtime recovery logic must be tested under failure, not just sunny-day paths.

- Wire client: packet drops, delays, malformed JSON, connection abort mid-byte.
- Gate execution: process `kill -9`, disk-full, permission-denied.
- Implement faults via configurable middleware / Tower layers or fakes, not by recompiling `mock_kimi` with new flags.
- **Rationale:** Netflix's Chaos Engineering and modern `fail-rs` / Tower fault injection are the standard. Without it, tests only verify that code works when everything goes right.

### Tier 7 — Mutation Testing

Coverage percentage is a necessary but insufficient metric. Mutation coverage measures whether tests actually **check** behavior.

- Run `cargo-mutants` in nightly CI.
- A module is considered "insured" if <5 % of mutants survive.
- A surviving mutant means either a missing test or dead code.
- **Rationale:** Mutation testing is the only objective measure of test quality. Line coverage only proves the code was *executed*; mutation coverage proves it was *verified*. Used in SQLite, curl, and increasingly in the Rust ecosystem.

### CI Tiers and Gates

| Tier | Tool | CI Frequency | Gate |
|---|---|---|---|
| 0 — Types | Type system + `static_assertions` | Every build | Hard (compilation) |
| 1 — Unit | `cargo nextest` | Every PR | Hard |
| 2 — Property | `proptest` | Every PR | Hard |
| 3 — Snapshot | `insta` | Every PR | Hard (review required) |
| 4 — Integration | `assert_cmd`, `tempfile` | Every PR | Hard |
| 5 — DST | `shuttle` / `turmoil` | Every PR (small) / Nightly (deep) | Hard |
| 6 — Fault injection | Custom middleware / fake | Nightly | Soft (report) |
| 7 — Mutation | `cargo-mutants` | Nightly | Soft (alert if >5 % survive) |
| Benchmarks | `criterion` | Nightly | Soft (regression alert) |

**Why `cargo nextest`:**
- Flaky-test detection with automatic retry and statistics.
- Test groups (serial vs parallel) replacing coarse global mutexes.
- Performance profiles per test (detect silent slowdowns).
- Standard across large Rust codebases (Mozilla, Ferrous Systems, Embark).

### Migration Rules (New Code vs Legacy)

**New code** (any newly created file or full rewrite):
- Must include Tier 0 (types) + Tier 1 (unit) + Tier 3 (snapshot, if any output).
- If it touches Wire / serde — add Tier 2 (property).
- If it touches concurrency — add Tier 5 (DST).

**Refactoring existing code:**
- When changing a function, add a property test if none exists.
- When changing output, replace manual `assert!(...contains(...))` with `insta` snapshots.
- When changing concurrent code, add a DST baseline test **before** the refactor, then refactor.

**No separate "add tests later" PRs.** Tests are part of the feature. PRs without new or updated tests do not pass review.

## Build & Test

Local verification commands:

```bash
# Fast feedback loop (parallel, flaky detection, test groups)
cargo nextest run

# Property-based and snapshot tests (requires installed tools)
cargo test --test "*" --features proptest
cargo insta test --accept

# Lint and type-check
cargo clippy --all-targets --all-features -- -D warnings
cargo check --all-targets --all-features
cargo doc --no-deps

# Coverage (optional local run; CI runs this automatically)
cargo llvm-cov --html
```

CI runs: `cargo nextest run`, `cargo test --doc`, `cargo clippy`, `cargo llvm-cov`, `cargo-deny`.

Enforced in CI. Warnings are treated as errors.

## Clippy Lint Policy

Enabled lints live in `src/lib.rs` via `#![warn(...)]`. They produce warnings
during compilation without breaking the build, but must be addressed before
merge. Some policy checks, such as `clippy::unwrap_used`, are intentionally not
crate-wide yet because legacy unit tests still use `unwrap()`; enforce them on
new or modified production code during review.

### Tier 1 — Must-fix (high value, low noise)

| Lint | What it catches | Rationale |
|---|---|---|
| `clippy::await_holding_lock` | `std::sync::Mutex` or `RefCell` held across `.await` | Prevents executor blocking and deadlocks in async code |
| `clippy::dbg_macro` | `dbg!()` left in committed code | Debugging macros must not reach production |
| `clippy::wildcard_imports` | `use module::*` outside preludes/tests | Keeps imports explicit and traceable |
| `clippy::unused_async` | `async fn` that does not `.await` anything | Removes unnecessary async overhead |

### Tier 2 — Recommended (address when touching nearby code)

| Lint | What it catches |
|---|---|
| `clippy::missing_panics_doc` | `expect()` / `panic!()` without doc comment explaining the invariant |
| `clippy::cast_sign_loss` | `as u64` from signed types (e.g. `i64 as u64`) |

### Tier 3 — Already clean (maintain zero violations)

- Zero `TODO` / `FIXME` / `HACK` comments in production code
- All public types implement `Debug`
- Zero `unsafe` blocks (currently 0 in `src/`)

## Editing Rules

- When modifying code, check whether a subdirectory has its own `AGENTS.md` for more specific guidance.
- Keep deeper-directory rules as overrides to these root rules.
- Update this file if you change any convention it describes.
- **File size hard limit: 400 lines.** Any file exceeding this limit must be split into a directory module (`foo.rs` → `foo/mod.rs` + focused submodules) following SRP. Preserve the public API via `pub use` re-exports in `mod.rs`.
