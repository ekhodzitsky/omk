# oh-my-kimi Agent Guide

This file contains agent-level conventions for the entire project tree.
For OMK-specific context (wire protocol, agent roles, roadmap), see `.omk/AGENTS.md`.

## Meta Principle

Before applying any rule or refactor, ask: **what problem does this solve?**
A newtype, a refactor, or an abstraction is justified only if it prevents a concrete bug,
clarifies an invariant, or removes a footgun. If the answer is "it looks better" — revert.
Decoration is not engineering.

## Project Contract Rules (Hard Constraints)

These rules protect OMK's most fragile contracts: the CLI surface, Kimi Wire
protocol boundary, async worker lifecycle, event/proof output, and release
documentation.

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
