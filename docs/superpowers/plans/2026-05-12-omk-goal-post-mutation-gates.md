# OMK Goal Post-Mutation Gate Rerun Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:test-driven-development for behavior changes. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** After the first bounded `goal-agent-execute` wave changes project files, rerun verification gates against the mutated tree before refreshing `proof.json`.

**Architecture:** `omk goal execute` runs the pre-agent local verification wall as before. If the Wire-backed agent task succeeds and changed files are detected, the controller runs the configured gates a second time under `artifacts/gates/post-mutation/`, appends gate events, updates the `goal-local-verify` task with the latest gate result, records `post_mutation_gates_ran` in `proof.json`, and removes the stale-gates known gap. Readiness still remains `not_ready` until review/security and integration acceptance are complete.

**Tech Stack:** Rust, Tokio, existing goal controller, gate runner, git changed-file detection, `MOCK_KIMI`, integration CLI tests.

---

### Task 1: Lock Post-Mutation Gate Behavior

**Files:**
- Modify: `tests/goal_cmd_test.rs`

- [x] **Step 1: Write the failing test**

Add an integration test that:
- creates a temp project with a passing custom gate;
- makes the gate write an external counter and print `before-agent` until `agent-output.txt` exists;
- runs `omk goal run`;
- runs `omk goal execute latest` with `MOCK_KIMI_WRITE_FILE=agent-output.txt`;
- asserts the gate counter records two runs;
- asserts final proof gate evidence reports `after-agent`;
- asserts the stale post-mutation-gates gap is absent.

- [x] **Step 2: Verify RED**

Run: `cargo test --test goal_cmd_test test_goal_execute_reruns_gates_after_agent_mutation`

Expected first result: FAIL because only the pre-agent gate run happened.

### Task 2: Runtime Integration

**Files:**
- Modify: `src/runtime/goal.rs`

- [x] **Step 3: Rerun gates after agent changes**

After successful agent execution, when changed files are detected, run configured gates again under `artifacts/gates/post-mutation/`.

- [x] **Step 4: Refresh proof from post-mutation evidence**

Use the post-mutation gate results, changed-file snapshot, and git evidence when writing the final `proof.json`.

- [x] **Step 5: Keep proof honest**

Add `post_mutation_gates_ran` to the proof model and keep the stale-gates gap only when changed files exist but no post-mutation gate pass has run.

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

Document that `omk goal execute` reruns verification gates when the agent wave changes project files.

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
