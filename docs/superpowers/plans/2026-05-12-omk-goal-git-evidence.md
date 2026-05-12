# OMK Goal Git Evidence Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Capture the git branch, HEAD commit, and dirty state in `omk goal` proof artifacts.

**Architecture:** Keep git evidence best-effort and local-only. If the project directory is not a git worktree, omit the git block and keep the proof valid. If git data is available, include a structured `git` object and keep the existing `commits` list populated with the current HEAD for compatibility.

**Tech Stack:** Rust, Tokio process execution, Serde JSON, integration CLI tests.

---

### Task 1: Lock Git Evidence Behavior

**Files:**
- Modify: `tests/goal_cmd_test.rs`

- [x] **Step 1: Write the failing test**

Add a test that creates a tiny git repository, commits a `.omk/gates.toml` and project file, runs `omk goal run`, then `omk goal verify latest`, and asserts:
- `proof.json.git.branch` is the active branch;
- `proof.json.git.head` is a 40-character SHA;
- `proof.json.git.dirty` is `false`;
- `proof.json.commits` contains the HEAD SHA.

- [x] **Step 2: Run focused test**

Run: `cargo test --test goal_cmd_test test_goal_verify_records_git_evidence`

Expected: FAIL because `GoalProof` does not capture git branch or commit evidence yet.

### Task 2: Runtime Git Evidence

**Files:**
- Modify: `src/runtime/goal.rs`

- [x] **Step 3: Add proof git model**

Add `GoalGitEvidence` with `branch`, `head`, and `dirty` fields, and add an optional `git` field to `GoalProof`.

- [x] **Step 4: Detect git evidence**

Use best-effort `git rev-parse --abbrev-ref HEAD`, `git rev-parse HEAD`, and `git status --porcelain` in the project directory. Return `None` outside git worktrees or when commands fail.

- [x] **Step 5: Attach git evidence to proofs**

Populate git evidence for scaffold proof creation and verification proof refreshes. Keep `commits` as `[head]` when a HEAD SHA is available.

### Task 3: Docs and Verification

**Files:**
- Modify: `README.md`
- Modify: `SPEC.md`
- Modify: `TODO.md`
- Modify: `docs/ARCHITECTURE.md`
- Modify: `CHANGELOG.md`

- [x] **Step 6: Update docs**

Document best-effort git evidence in goal proofs.

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
