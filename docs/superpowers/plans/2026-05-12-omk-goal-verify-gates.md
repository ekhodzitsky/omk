# OMK Goal Verify Gates Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `omk goal verify` so goal proofs contain real local verification gate evidence while staying honest about missing execution.

**Architecture:** Reuse existing gate detection/configuration and gate evidence artifact writing. The goal runtime updates `proof.json`, `goal.json`, and `events.jsonl`; the CLI exposes a compact `verify` command. Passing gates remove the "verification gates have not run" gap but do not make a goal `ready` until agent/task execution is implemented.

**Tech Stack:** Rust, Clap, Tokio, existing `runtime::gates`, existing event builder/writer, Serde JSON.

---

### Task 1: Lock Verify Behavior

**Files:**
- Modify: `tests/goal_cmd_test.rs`

- [x] **Step 1: Write failing tests**

Add tests that:
- create a temp project with `.omk/gates.toml`;
- run `omk goal run ...`;
- run `omk goal verify latest`;
- assert gate output artifacts exist under `artifacts/gates`;
- assert `proof.json` contains gate results and remains `not_ready`;
- assert failing required gates keep status `not_ready`.

- [x] **Step 2: Run focused test**

Run: `cargo test --test goal_cmd_test`

Expected: FAIL because `omk goal verify` is not implemented.

### Task 2: Runtime Gate Verification

**Files:**
- Modify: `src/runtime/goal.rs`

- [x] **Step 3: Store gate results in goal proof**

Change `GoalProof.gates` from strings to `GateResult` values.

- [x] **Step 4: Add `verify_goal`**

Load or detect gates from the current project directory, run them with evidence under `goals/<id>/artifacts/gates`, append command/gate events, update changed files and known gaps, write `proof.json`, and save `goal.json`.

### Task 3: CLI and Docs

**Files:**
- Modify: `src/cli/goal.rs`
- Modify: `README.md`
- Modify: `SPEC.md`
- Modify: `TODO.md`
- Modify: `docs/ARCHITECTURE.md`
- Modify: `docs/PROJECT_MAP.md`
- Modify: `CHANGELOG.md`

- [x] **Step 5: Add `omk goal verify`**

Expose `omk goal verify [goal-id|latest]`.

- [x] **Step 6: Update documentation**

Document that `goal verify` runs real gates, writes evidence, and still requires future task execution before `ready`.

### Task 4: Verification and Ship

- [x] **Step 7: Run verification**

Run:
- `git diff --check`
- `cargo fmt -- --check`
- `cargo check --all-targets`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test`
- `cargo doc --no-deps`

- [x] **Step 8: Commit and push**

Use the Lore commit protocol, push to `master`, and wait for GitHub CI/Coverage.
