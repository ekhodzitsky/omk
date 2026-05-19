# oh-my-kimi Agent Guide

This file contains agent-level conventions for the entire project tree.
For OMK-specific context (wire protocol, agent roles, roadmap), see `.omk/AGENTS.md`.
For the main product direction, read `SPEC.md`, `ROADMAP.md`, and `TODO.md`.
For market positioning and competitor boundaries, read
`docs/COMPETITIVE_POSITIONING.md`.

## Meta Principle

Before applying any rule or refactor, ask: **what problem does this solve?**
A newtype, a refactor, or an abstraction is justified only if it prevents a concrete bug,
clarifies an invariant, or removes a footgun. If the answer is "it looks better" — revert.
Decoration is not engineering.

## Behavioral Guidelines

Behavioral guidelines to reduce common LLM coding mistakes. Merge with project-specific instructions as needed.

**Tradeoff:** These guidelines bias toward caution over speed. For trivial tasks, use judgment.

### 1. Think Before Coding

**Don't assume. Don't hide confusion. Surface tradeoffs.**

Before implementing:

- State your assumptions explicitly. If uncertain, ask.
- If multiple interpretations exist, present them — don't pick silently.
- If a simpler approach exists, say so. Push back when warranted.
- If something is unclear, stop. Name what's confusing. Ask.

### 2. Simplicity First

**Minimum code that solves the problem. Nothing speculative.**

- No features beyond what was asked.
- No abstractions for single-use code.
- No "flexibility" or "configurability" that wasn't requested.
- No error handling for impossible scenarios.
- If you write 200 lines and it could be 50, rewrite it.

Ask yourself: "Would a senior engineer say this is overcomplicated?" If yes, simplify.

### 3. Surgical Changes

**Touch only what you must. Clean up only your own mess.**

When editing existing code:

- Don't "improve" adjacent code, comments, or formatting.
- Don't refactor things that aren't broken.
- Match existing style, even if you'd do it differently.
- If you notice unrelated dead code, mention it — don't delete it.

When your changes create orphans:

- Remove imports/variables/functions that YOUR changes made unused.
- Don't remove pre-existing dead code unless asked.

The test: Every changed line should trace directly to the user's request.

### 4. Goal-Driven Execution

**Define success criteria. Loop until verified.**

Transform tasks into verifiable goals:

- "Add validation" → "Write tests for invalid inputs, then make them pass"
- "Fix the bug" → "Write a test that reproduces it, then make it pass"
- "Refactor X" → "Ensure tests pass before and after"

For multi-step tasks, state a brief plan:

```
1. [Step] → verify: [check]
2. [Step] → verify: [check]
3. [Step] → verify: [check]
```

Strong success criteria let you loop independently. Weak criteria ("make it work") require constant clarification.

---

**These guidelines are working if:** fewer unnecessary changes in diffs, fewer rewrites due to overcomplication, and clarifying questions come before implementation rather than after mistakes.

## Project Contract Rules (Hard Constraints)

These rules protect OMK's most fragile contracts: the CLI surface, Kimi Wire
protocol boundary, async worker lifecycle, event/proof output, and release
documentation.

`omk goal` is the north-star feature. It must be designed as a proof-driven
controller over existing Wire/team/event/proof primitives, not as an unbounded
recursive agent launcher.
Position it as a local, repo-native, proof-driven autonomous software
engineering runtime, not as a Devin clone, generic app builder, or IDE chat.

1. **`src/main.rs` stays thin.** The binary crate must only call the library
   entrypoint (currently `omk::cli::run().await`). Do not declare project modules
   or put command dispatch/business logic in the binary crate.
2. **Public API is opt-in.** Prefer `pub(crate)`. New `pub`, `pub mod`, or
   `pub use` items require a concrete external caller or a paved-path rationale,
   plus an integration test when the API is meant to be imported by users/tests.
3. **Wire protocol changes require compatibility evidence.** Any change under
   `src/wire/` must include serde roundtrip or golden coverage, unknown/extra
   field behavior where relevant, and redaction coverage for secret-like fields.
4. **Protocol facts must not go stale.** If `KIMI_WIRE_PROTOCOL_VERSION`, event
   names, request names, or Kimi CLI behavior changes, update `.omk/AGENTS.md`,
   README/docs, changelog, and tests in the same change set.
5. **Async workers need explicit ownership.** Every spawned task, child process,
   Wire worker, scheduler loop, or background watcher must document in code shape
   who cancels it, who joins/aborts it, and what event/proof evidence is emitted
   on stop/failure.
6. **Machine channels and human logs stay separate.** Do not write human logs to
   Wire/stdout protocol streams, JSONL event streams, proof JSON, or other
   machine-readable output unless that text is part of the documented schema.
7. **Events, metrics, and proof fields are public API.** Renames/removals need a
   compatibility alias, migration note, or explicit changelog entry explaining
   the break.
8. **CLI UX is testable contract.** New commands/flags or changed user-facing
   behavior require smoke coverage for `--help`, exit status, errors, and
   machine-readable output when JSON/Markdown/proof output is involved.
9. **Dependencies are architecture changes.** No new crate without a rationale:
   why std/local code is not enough, transitive impact, MSRV, license, and
   feature-flag consequences. Small helper crates are rejected by default.
10. **Refactors isolate mechanics from behavior.** File moves, splits, renames,
    and formatting-only changes must be separate from semantic changes whenever
    practical.
11. **Tests must be deterministic.** Avoid naked sleeps. Async tests should wait
    for observable state under a timeout. `#[ignore]` requires a clear reason and
    a tracking note.
12. **Secrets are redacted at boundaries.** Logs, events, proof/failure artifacts,
    MCP output, and debug dumps must pass through centralized redaction when
    carrying token/key/secret/auth-like data.

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

## Build & Test

```bash
cargo test
cargo clippy --all-targets --all-features -- -D warnings
cargo doc --no-deps
```

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
