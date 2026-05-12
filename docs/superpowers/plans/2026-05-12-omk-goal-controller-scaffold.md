# OMK Goal Controller Scaffold Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the first `omk goal` controller scaffold that writes planning, task graph, and proof artifacts without pretending agent execution exists.

**Architecture:** Keep the implementation inside the existing `src/runtime/goal.rs` and `src/cli/goal.rs` boundary. `omk goal run` and `omk goal plan` create the same durable scaffold: `prd.md`, `technical-plan.md`, `test-spec.md`, `task-graph.json`, and `proof.json`. `omk goal proof` reads the proof artifact in text, JSON, or Markdown.

**Tech Stack:** Rust, Clap, Tokio file IO, Serde JSON, existing OMK atomic writes and event writer.

---

### Task 1: Lock Controller Scaffold Behavior

**Files:**
- Modify: `tests/goal_cmd_test.rs`

- [x] **Step 1: Write failing CLI tests**

Add tests that expect:
- `omk goal run` writes `prd.md`, `technical-plan.md`, `test-spec.md`, `task-graph.json`, and `proof.json`.
- `omk goal show latest --json` includes `phase: "proof"` and artifact records.
- `omk goal proof latest --json` returns `status: "not_ready"` with an execution gap.
- `omk goal plan <goal>` creates the same scaffold.

- [x] **Step 2: Run test to verify it fails**

Run: `cargo test --test goal_cmd_test`

Expected: FAIL because `goal proof` and `goal plan` do not exist and scaffold artifacts are not written yet.

### Task 2: Implement Runtime Controller Scaffold

**Files:**
- Modify: `src/runtime/goal.rs`

- [x] **Step 3: Add typed scaffold models**

Add:
- `GoalPhase`
- `GoalArtifact`
- `GoalTaskStatus`
- `GoalTask`
- `GoalTaskGraph`
- `GoalProof`

- [x] **Step 4: Write scaffold artifacts**

Refactor goal creation so the controller writes:
- `prd.md`
- `technical-plan.md`
- `test-spec.md`
- `task-graph.json`
- `proof.json`

The status remains `not_ready` because no agent execution or verification gates have run.

- [x] **Step 5: Run focused tests**

Run: `cargo test --test goal_cmd_test`

Expected: PASS.

### Task 3: Add CLI Surfaces

**Files:**
- Modify: `src/cli/goal.rs`

- [x] **Step 6: Add `plan` and `proof` commands**

Add:
- `omk goal plan <goal>`
- `omk goal proof [goal-id|latest] [--format text|json|md] [--json]`

- [x] **Step 7: Update `show` output**

Include phase, artifacts, and proof path in text/Markdown output.

### Task 4: Documentation and Verification

**Files:**
- Modify: `README.md`
- Modify: `SPEC.md`
- Modify: `TODO.md`
- Modify: `docs/ARCHITECTURE.md`
- Modify: `docs/PROJECT_MAP.md`
- Modify: `CHANGELOG.md`

- [x] **Step 8: Update docs**

Document that the current `omk goal` scaffold now writes planning artifacts, a task graph, and an honest not-ready proof.

- [x] **Step 9: Run verification**

Run:
- `git diff --check`
- `cargo fmt -- --check`
- `cargo check --all-targets`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test`
- `cargo doc --no-deps`

- [x] **Step 10: Commit and push**

Use the Lore commit protocol, push to `master`, then wait for GitHub CI and Coverage.
