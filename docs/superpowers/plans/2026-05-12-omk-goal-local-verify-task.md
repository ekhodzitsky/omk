# OMK Goal Local Verify Task Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make local verification a real `omk goal` task graph step with task evidence, while leaving future agent execution pending.

**Architecture:** Split the previous combined `goal-execute-verify` placeholder into `goal-local-verify` and `goal-agent-execute`. Add `omk goal execute [goal-id|latest]` as a controller step that runs the same verification wall, updates the task graph with local verification evidence, refreshes proof, and stays `not_ready` until agent execution exists.

**Tech Stack:** Rust, Clap, Tokio, Serde JSON, existing gate runner, integration CLI tests.

---

### Task 1: Lock Local Verification Task Behavior

**Files:**
- Modify: `tests/goal_cmd_test.rs`

- [x] **Step 1: Write the failing test**

Add `test_goal_execute_records_local_verification_task_evidence`, which:
- creates a temp project with a passing `.omk/gates.toml`;
- runs `omk goal run "Execute local verification evidence"`;
- runs `omk goal execute latest`;
- asserts CLI output reports `not_ready`, `goal-local-verify`, and `goal-agent-execute`;
- asserts `task-graph.json` has four tasks;
- asserts `goal-local-verify` is `done` with controller-owned evidence for gate artifacts and `proof.json`;
- asserts `goal-agent-execute` remains `pending`;
- asserts `proof.json` reports three done tasks and one pending task.

- [x] **Step 2: Run focused test**

Run: `cargo test --test goal_cmd_test test_goal_execute_records_local_verification_task_evidence`

Expected: FAIL because `omk goal execute` and the split task graph do not exist yet.

### Task 2: Runtime and CLI Execution Step

**Files:**
- Modify: `src/runtime/goal.rs`
- Modify: `src/cli/goal.rs`

- [x] **Step 3: Split the scaffold task graph**

Replace `goal-execute-verify` with:
- `goal-local-verify`: pending, depends on `goal-plan`, accepted by required gates passing and proof refresh;
- `goal-agent-execute`: pending, depends on `goal-local-verify`, accepted only by future agent execution evidence.

- [x] **Step 4: Add execution proof refresh**

Add `execute_goal(goal_id, project_dir)` that runs gates, records git and changed-file evidence, marks `goal-local-verify` done when required gates pass, writes task events, saves `task-graph.json`, refreshes `proof.json`, and leaves `goal-agent-execute` pending.

- [x] **Step 5: Add CLI command**

Expose `omk goal execute [goal-id|latest]` and print the local verification task status plus remaining agent execution state.

### Task 3: Docs and Verification

**Files:**
- Modify: `README.md`
- Modify: `SPEC.md`
- Modify: `TODO.md`
- Modify: `CHANGELOG.md`
- Modify: `docs/ARCHITECTURE.md`
- Modify: `docs/PROJECT_MAP.md`

- [x] **Step 6: Update docs**

Document that `execute` currently performs the local verification wall and task evidence update, but does not launch agents yet.

- [x] **Step 7: Run verification**

Run:
- `git diff --check`
- `cargo fmt -- --check`
- `cargo check --all-targets`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --test goal_cmd_test`
- `cargo test`
- `cargo doc --no-deps`
- `cargo deny --all-features check advisories licenses`

- [x] **Step 8: Commit, push, and watch CI**

Use the Lore commit protocol, push to `master`, and wait for GitHub CI/Coverage.
