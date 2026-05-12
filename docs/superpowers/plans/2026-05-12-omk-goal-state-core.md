# OMK Goal State Core Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the first usable `omk goal` skeleton: durable goal state, basic CLI inspection, and cancellation.

**Architecture:** Keep this slice intentionally small. Add a `runtime::goal` module for state files and a `cli::goal` module for command handling. Do not launch agents yet; `run` creates inspectable state and exits with a truthful `not_ready` scaffold status unless cancelled later.

**Tech Stack:** Rust, Clap, Tokio, Serde JSON, existing `runtime::atomic`, existing XDG state path helpers, `assert_cmd` integration tests.

---

### Task 1: Failing CLI Smoke Tests

**Files:**
- Create: `tests/goal_cmd_test.rs`

- [x] **Step 1: Write failing tests**

Test `omk goal --help`, `omk goal run`, `status latest`, `show latest --format json`, `list`, and `cancel latest`.

- [x] **Step 2: Run tests to verify RED**

Run: `cargo test --test goal_cmd_test`
Expected: FAIL because `goal` command is not implemented.

### Task 2: Runtime Goal State

**Files:**
- Create: `src/runtime/goal.rs`
- Modify: `src/runtime/mod.rs`

- [x] **Step 1: Implement state types**

Add `GoalStatus`, `GoalState`, `GoalTerminalCriteria`, and helpers for creating,
loading, listing, resolving latest, and cancelling goal runs.

- [x] **Step 2: Persist exact files**

Create `goals/<goal-id>/goal.json` under the OMK state directory and write
`events.jsonl`. Cancel writes `failure.json` with status and reason.

- [x] **Step 3: Run runtime-focused tests**

Run: `cargo test runtime::goal`
Expected: PASS.

### Task 3: CLI Goal Module

**Files:**
- Create: `src/cli/goal.rs`
- Modify: `src/cli/mod.rs`
- Modify: `src/cli/app.rs`

- [x] **Step 1: Add Clap surface**

Commands:

```bash
omk goal run <goal> [--until-ready] [--budget-time <duration>] [--max-agents <n>]
omk goal list
omk goal status [goal-id|latest]
omk goal show [goal-id|latest] [--format text|json|md] [--json]
omk goal cancel [goal-id|latest]
```

- [x] **Step 2: Keep output honest**

`run` must say this is the scaffold and returns `not_ready` state. `show --json`
must emit machine-readable `GoalState`.

- [x] **Step 3: Run CLI tests**

Run: `cargo test --test goal_cmd_test`
Expected: PASS.

### Task 4: Documentation and Release Notes

**Files:**
- Modify: `README.md`
- Modify: `SPEC.md`
- Modify: `TODO.md`
- Modify: `CHANGELOG.md`

- [x] **Step 1: Mark maturity accurately**

Document `omk goal` as `Current Scaffold`, not MVP or ready autonomous execution.

- [x] **Step 2: Check doc line limits**

Run: `wc -l README.md SPEC.md TODO.md CHANGELOG.md docs/superpowers/plans/2026-05-12-omk-goal-state-core.md`
Expected: touched docs remain under 400 lines.

### Task 5: Verification and Commit

- [x] **Step 1: Run verification**

Run:

```bash
git diff --check
cargo fmt -- --check
cargo check --all-targets
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

- [x] **Step 2: Commit and push**

Use Lore commit trailers. Mention that this is scaffold state, not autonomous
execution.
