# OMK Goal Controller Task Evidence Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the `omk goal` scaffold truthfully record controller-owned task completion evidence before real agent execution exists.

**Architecture:** Keep execution claims narrow. The goal controller may mark only tasks it actually performs (`goal-intake` and `goal-plan`) as `done`, attach artifact evidence to those tasks, and append task events from the `goal-controller` actor. The future agent execution task remains `pending`, and goal proof remains `not_ready`.

**Tech Stack:** Rust, Serde JSON, existing goal runtime, existing event writer, integration CLI tests.

---

### Task 1: Lock Controller-Owned Task Evidence

**Files:**
- Modify: `tests/goal_cmd_test.rs`

- [x] **Step 1: Write the failing test**

Update the goal task graph test so it asserts:
- `goal-intake` is `done`;
- `goal-plan` is `done`;
- `goal-execute-verify` remains `pending`;
- done tasks carry artifact evidence;
- `proof.json.task_graph_summary` reports two done tasks and one pending task;
- `events.jsonl` contains `task_completed` events from `goal-controller`.

- [x] **Step 2: Run focused test**

Run: `cargo test --test goal_cmd_test test_goal_run_writes_task_graph_and_not_ready_proof`

Expected: FAIL because all scaffold tasks are currently written as `pending`.

### Task 2: Persist Task Evidence in the Runtime

**Files:**
- Modify: `src/runtime/goal.rs`

- [x] **Step 3: Add task evidence fields**

Add a small `GoalTaskEvidence` struct and backward-compatible `GoalTask` fields:
- `owner_role: Option<String>`;
- `completed_at: Option<DateTime<Utc>>`;
- `evidence: Vec<GoalTaskEvidence>`.

- [x] **Step 4: Mark only controller-owned work as done**

When writing `task-graph.json`, mark `goal-intake` and `goal-plan` as `done` with artifact evidence. Keep `goal-execute-verify` pending.

- [x] **Step 5: Append task completion events**

Append `task_started` and `task_completed` events for the controller-owned tasks with actor `goal-controller` and summaries that cite the generated artifacts.

### Task 3: Docs and Verification

**Files:**
- Modify: `README.md`
- Modify: `SPEC.md`
- Modify: `TODO.md`
- Modify: `docs/ARCHITECTURE.md`
- Modify: `docs/PROJECT_MAP.md`
- Modify: `CHANGELOG.md`

- [x] **Step 6: Update docs**

Document that the scaffold now records controller-owned task evidence, while real agent execution remains next.

- [x] **Step 7: Run verification**

Run:
- `git diff --check`
- `cargo fmt -- --check`
- `cargo check --all-targets`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test`
- `cargo doc --no-deps`

- [ ] **Step 8: Commit and push**

Use the Lore commit protocol, push to `master`, and wait for GitHub CI/Coverage.
