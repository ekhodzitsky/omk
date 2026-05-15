# Code Quality Cleanup Design

Date: 2026-05-15

Status: design approved

## Summary

Systematic removal of `unwrap()`, `expect()`, and `panic!()` from production code in `src/`, followed by splitting files exceeding 400 lines into directory modules per SRP. Zero clippy warnings, all tests passing. No behavioral changes.

## Problem

Current production code under `src/` (outside `#[cfg(test)]`) contains:
- 580 `unwrap()` calls
- 28 `expect()` calls
- 8 `panic!()` calls
- 16 files exceeding 400 lines (AGENTS.md hard limit)

These violate OMK's Rust Safety Rules (hard constraints) and create footguns in async runtime code.

## Approach

**Approach A: «Механика сначала»** — mechanical fixes first, structural splits second.

## Phases

### Phase 1: Remove `unwrap()` (580 occurrences)

- Work module-by-module (not file-by-file) for testable increments
- Replace with: `?`, `if let`, `match`, `ok_or`, `bail!`, `.context()`
- If function lacks `Result` return: add `Result` to signature or wrap call site
- Rule: no new `expect()` as replacement — use `?` propagation or `.context()`
- One commit per module
- Gate: `cargo test` + `cargo clippy --all-targets --all-features -- -D warnings` must pass

### Phase 2: Remove `expect()` (28 occurrences)

- Read existing message for intent
- Replace with `.context("message")?` or `bail!("message")`
- One commit per module
- Same gate as Phase 1

### Phase 3: Remove `panic!()` (8 occurrences)

- Most complex — may require adding `Result` to call chain
- `unreachable!()` patterns: `debug_assert!` + `return Err(...)`
- Graceful degradation preferred over crashing
- One commit per module
- Same gate as Phase 1

### Phase 4: Split files >400 lines (16 files)

- Convert `foo.rs` → `foo/mod.rs` + focused submodules
- Preserve public API via `pub use` re-exports in `mod.rs`
- No logic changes — pure mechanical moves
- One commit per file
- Same gate as Phase 1

## Files >400 Lines (Target List)

| File | Lines |
|------|-------|
| src/runtime/goal/dispatch.rs | 800 |
| src/runtime/autopilot/engine.rs | 672 |
| src/kimi_native/manifest.rs | 649 |
| src/cli/app.rs | 635 |
| src/runtime/goal/verifier.rs | 598 |
| src/vis/hud.rs | 555 |
| src/runtime/goal/state.rs | 542 |
| src/vis/server.rs | 520 |
| src/runtime/ralph.rs | 517 |
| src/kimi_native/diagnostics.rs | 468 |
| src/runtime/proof/generator.rs | 454 |
| src/vis/hud_tui.rs | 453 |
| src/wire/protocol/event.rs | 452 |
| src/runtime/ask.rs | 419 |
| src/runtime/scheduler/claim.rs | 413 |
| src/runtime/scheduler/runner/tests.rs | 412 |

## Testing Strategy

After each phase and each commit:
```bash
cargo test
cargo clippy --all-targets --all-features -- -D warnings
cargo doc --no-deps
```

All must pass. If broken — revert and retry.

## Commit Strategy

- Branch: `refactor/code-quality-cleanup`
- One commit = one module (phases 1–3) or one file (phase 4)
- Messages:
  - `refactor(<module>): remove unwrap/expect/panic from <module>`
  - `refactor(<file>): split into submodules per SRP`

## Risks

| Risk | Mitigation |
|------|------------|
| Tests break after signature change | Run tests after every module |
| `?` propagation changes error types | Use `.context()` to preserve semantic messages |
| File split breaks imports | Verify `pub use` re-exports compile |
| Scope creep | Strictly mechanical — no feature changes |

## Non-Goals

- No new features
- No performance optimization
- No API changes (signatures may gain `Result`, but semantics stay)
- No changes to `#[cfg(test)]` code
