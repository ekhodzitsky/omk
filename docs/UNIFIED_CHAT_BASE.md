# UNIFIED CHAT — Base SHA

> **Purpose:** Record the exact `master` HEAD from which the six Wave-1 workstream branches (`feat/chat-shell`, `feat/intent-classifier`, `feat/router`, `feat/engine-pane`, `feat/control-surface`, `feat/goal-as-engine`) were created.
>
> This file is referenced by `docs/UNIFIED_CHAT.md` §14.1 and `docs/UNIFIED_CHAT_DECISIONS.md`.

## Snapshot

- **Base SHA (full):** `8425033d76fb9d515f60fc6c901de004bda04526`
- **Base SHA (short):** `8425033`
- **Captured at:** 2026-05-21 (UTC+3)
- **Subject of base commit:** `feat(review): architect pass with file-size and cross-module import rules (#112)`

## Branches anchored to this base

| Workstream | Branch                  | Worktree                                              |
| ---------- | ----------------------- | ----------------------------------------------------- |
| W1         | `feat/chat-shell`       | `.worktrees/unified-chat-W1-shell`                    |
| W2         | `feat/intent-classifier`| `.worktrees/unified-chat-W2-classifier`               |
| W3         | `feat/router`           | `.worktrees/unified-chat-W3-router`                   |
| W4         | `feat/engine-pane`      | `.worktrees/unified-chat-W4-engine-pane`              |
| W5         | `feat/control-surface`  | `.worktrees/unified-chat-W5-control-surface`          |
| W6         | `feat/goal-as-engine`   | `.worktrees/unified-chat-W6-goal-bridge`              |

## Rebase policy

Per `UNIFIED_CHAT.md` §14.1, the orchestrator rebases each workstream PR onto current `master` weekly. Workstream worker agents must NOT rebase their own branches — only the orchestrator does, in a controlled pass, to keep merge state predictable.

## Merge order (from §14.8)

Wave-1 workstreams merge in this order, contingent on each previous merge being green:

1. **W1** (`feat/chat-shell`) — foundation, nothing else can land first.
2. **W2** (`feat/intent-classifier`) — independent.
3. **W4** (`feat/engine-pane`) — needs W1.
4. **W6** (`feat/goal-as-engine`) — mergeable in parallel with W4.
5. **W3** (`feat/router`) — needs W2, W6.
6. **W5** (`feat/control-surface`) — needs W1, W3, W6.

## Co-existing in-flight workstreams (NOT part of Wave 1)

At base-SHA time, these audit/quality workstreams are also in flight against `master`. They are NOT part of UNIFIED_CHAT Wave 1, but the orchestrator must avoid path collisions:

- `ws/goal-auto-rebase-on-merge-tree` (WS-01) — `src/runtime/goal/worktree/`, `delivery/slice_pr/rebase.rs`, `control/until_ready/git.rs`, new `git_ops/auto_rebase.rs`
- `ws/goal-supervisor-concurrent-test` (WS-04) — `tests/goal_supervisor_concurrent_test.rs`
- `ws/github-branch-protection-automation` (WS-05) — new `src/runtime/goal/delivery/github_api.rs`, `open_pr/release.rs`, `src/cli/goal/mod.rs`
- `ws/review-architect-real-rules` (WS-06) — **already merged** as #112 (this base SHA)
- `ws/review-security-auto-fix` (WS-09) — `src/runtime/goal/lifecycle/cleanup.rs`, `verifier/security.rs`
- `ws/cargo-toml-bench-example-entries` (WS-28) — `Cargo.toml`
- `ws/ci-restore-ubuntu-build` (WS-29) — `.github/workflows/ci.yml`

W6 (goal-as-engine) is the only Wave-1 workstream that touches `src/runtime/goal/`. W6's worker is instructed to confine its additions to a NEW subdirectory (`src/runtime/goal/chat_api/`) plus minimal `mod.rs` registration, to keep collision surface with WS-01/WS-05/WS-09 limited to one line in `src/runtime/goal/mod.rs`.
