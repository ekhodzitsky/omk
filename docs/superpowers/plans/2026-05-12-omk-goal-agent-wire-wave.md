# OMK Goal Agent Wire Wave Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:test-driven-development for behavior changes. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `goal-agent-execute` run through the existing scheduler and Wire worker adapter, producing agent-owned evidence without claiming full production readiness yet.

**Architecture:** `omk goal execute` first runs the local verification wall. If required gates pass, it creates a bounded agent run under the goal artifacts directory, starts one Wire worker, dispatches the `goal-agent-execute` scheduler task, records worker artifacts, updates the goal task graph, then refreshes the proof. This slice originally blocked readiness on review/security evidence; follow-up slices now capture mutation evidence, rerun gates after mutation, and block readiness until integration acceptance exists.

**Tech Stack:** Rust, Tokio, existing scheduler, existing Wire worker adapter, `MOCK_KIMI`, integration CLI tests.

---

### Task 1: Lock Agent Evidence Behavior

**Files:**
- Modify: `tests/goal_cmd_test.rs`

- [x] **Step 1: Write the failing test**

Add an integration test that:
- creates a temp project with a passing `.omk/gates.toml`;
- runs `omk goal run`;
- runs `omk goal execute latest` with `MOCK_KIMI` pointing at the fixture binary;
- asserts `goal-local-verify` and `goal-agent-execute` both become `done`;
- asserts task evidence points at the agent run directory, worker outbox, and Wire events;
- asserts proof no longer says agent execution is not implemented, but still stays `not_ready` for missing review evidence.

- [x] **Step 2: Run focused test**

Run: `cargo test --test goal_cmd_test test_goal_execute_runs_mock_wire_agent_and_records_agent_task_evidence`

Expected: FAIL because `execute_goal` only runs local verification today.

### Task 2: Runtime Integration

**Files:**
- Modify: `src/runtime/goal.rs`
- Modify: `src/runtime/wire_worker.rs`
- Modify: `src/runtime/wire_worker/loop_impl.rs`

- [x] **Step 3: Add fast test poll support**

Add an environment-controlled Wire worker poll duration for tests while preserving the production default.

- [x] **Step 4: Add scheduler-backed agent execution**

After successful local gates, create an agent run directory, save a `WorkerSpec`, start `WireWorkerAdapter`, initialize `TeamRunner` with one `goal-agent-execute` task, run it to completion, cancel the adapter, and convert the run summary into goal task evidence.

- [x] **Step 5: Keep proof honest**

When agent execution evidence exists, remove the old "not implemented" gap and replace readiness language with the next missing evidence class: review/security/integration-hardening evidence.

### Task 3: Docs and Verification

**Files:**
- Modify: `README.md`
- Modify: `SPEC.md`
- Modify: `TODO.md`
- Modify: `CHANGELOG.md`
- Modify: `docs/ARCHITECTURE.md`
- Modify: `docs/PROJECT_MAP.md`
- Modify: `docs/goal-runtime.md`
- Modify: `docs/goal-runtime-plan.md`
- Modify: `crates/omk-cli/README.md`

- [x] **Step 6: Update docs**

Document that `omk goal execute` now launches a bounded Wire-backed agent wave when gates pass, but readiness remains blocked until review/security evidence, post-mutation verification, and integration acceptance exist.

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
