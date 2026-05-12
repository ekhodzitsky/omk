# OMK Goal Agent Mutation Evidence Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:test-driven-development for behavior changes. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let the first `goal-agent-execute` wave make bounded project changes and record concrete mutation evidence without claiming production readiness.

**Architecture:** `omk goal execute` still runs required gates before launching the Wire-backed worker. The worker prompt now allows minimal in-repo mutations, the controller captures a post-worker changed-file snapshot and mutation diff under `artifacts/agent-runs/goal-agent-execute/`, and `proof.json` remains `not_ready` until post-mutation gates, review, security, and integration acceptance run against the changed tree.

**Tech Stack:** Rust, Tokio, existing goal controller, existing Wire worker adapter, git status/diff, `MOCK_KIMI`, integration CLI tests.

---

### Task 1: Lock Mutation Evidence Behavior

**Files:**
- Modify: `tests/goal_cmd_test.rs`
- Modify: `tests/fixtures/mock_kimi.rs`

- [x] **Step 1: Write the failing test**

Add an integration test that:
- creates a temp project with a passing `.omk/gates.toml`;
- commits a clean baseline;
- runs `omk goal run`;
- runs `omk goal execute latest` with `MOCK_KIMI_WRITE_FILE=agent-output.txt`;
- asserts the worker-created file exists;
- asserts task evidence includes `mutation.diff` and `changed-files.json`;
- asserts proof changed files include `agent-output.txt`;
- asserts proof stays `not_ready` because gates have not rerun after the agent mutation.

- [x] **Step 2: Run focused test**

Run: `cargo test --test goal_cmd_test test_goal_execute_records_agent_mutation_diff_when_worker_changes_project_files`

Expected first result: FAIL because the worker did not mutate the project or record mutation evidence yet.

### Task 2: Runtime Integration

**Files:**
- Modify: `src/runtime/goal.rs`
- Modify: `src/runtime/gates/detect.rs`

- [x] **Step 3: Allow bounded worker mutations**

Adjust the goal-agent prompt so the worker may make minimal in-repo changes, while keeping the controller boundary clear: no commits, no publishing, no secrets, and no broad rewrites.

- [x] **Step 4: Capture mutation artifacts**

After the worker run, capture:
- `artifacts/agent-runs/goal-agent-execute/mutation.diff`
- `artifacts/agent-runs/goal-agent-execute/changed-files.json`

Use git status for changed-file detection so untracked worker output is visible.

- [x] **Step 5: Keep proof honest**

When changed files exist after agent execution, keep `proof.json` `not_ready` with a known gap for post-mutation verification and integration. When no changed files exist, keep a separate no-mutation gap.

### Task 3: Docs and Verification

**Files:**
- Modify: `README.md`
- Modify: `SPEC.md`
- Modify: `TODO.md`
- Modify: `ROADMAP.md`
- Modify: `CHANGELOG.md`
- Modify: `docs/ARCHITECTURE.md`
- Modify: `docs/PROJECT_MAP.md`
- Modify: `docs/API.md`
- Modify: `src/cli/README.md`

- [x] **Step 6: Update docs**

Document that `omk goal execute` now captures mutation diff and changed-file evidence, but readiness remains blocked until post-mutation gates, review/security, and integration acceptance.

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
- `cargo package --allow-dirty`

- [x] **Step 8: Commit, push, and watch CI**

Use the Lore commit protocol, push to `master`, and wait for GitHub CI/Coverage.
