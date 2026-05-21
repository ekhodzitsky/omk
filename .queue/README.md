# `.queue/` — Orchestrator task queue

This directory is the **canonical source of truth** for parallel-dispatch work in this repo. Each `.md` file is one task with one prompt that one worker agent picks up and executes in its own worktree.

## Design rules

1. **One file = one task = one prompt = one PR.** No bundling.
2. **Numeric ID = filename prefix.** Three-digit zero-padded (`001`, `010`, `123`). IDs are sticky for life; never reassigned even if a task is cancelled.
3. **Self-contained prompt body.** A worker needs nothing beyond this file plus the repo to execute the task.
4. **YAML front matter holds machine-readable metadata.** Human-readable prose lives below the second `---`.
5. **Tasks are git-tracked.** Status changes are commits. History is auditable.

## File format

```markdown
---
id: 010
title: W1 — chat TUI shell foundation
status: todo                # todo | wip | pr_open | done | cancelled
branch: feat/chat-shell
worktree: .worktrees/unified-chat-W1-shell
blocked_by: []              # cannot START until these IDs are done
merge_after: []             # CAN start, but cannot MERGE until these are done
size: large                 # small | medium | large
batch: unified-chat-wave-1  # logical grouping
pr: null                    # PR number once opened
notes: null                 # optional one-line orchestrator note
---

# Prompt body

(everything after the second `---` is the verbatim prompt for the worker agent)
```

## Status taxonomy

| Status     | Meaning                                                                 |
| ---------- | ----------------------------------------------------------------------- |
| `todo`     | Ready to claim. All `blocked_by` are `done`.                            |
| `wip`      | Worker has claimed; PR not yet open. `branch` is being worked.          |
| `pr_open`  | PR exists, awaiting review/CI/merge. `pr` field set.                    |
| `done`     | PR merged to master.                                                    |
| `cancelled`| Abandoned. Reason in `notes`.                                           |

A task is **eligible for pickup** iff `status == todo` AND every `blocked_by` ID resolves to a task with `status == done`.

## Worker lifecycle

1. Worker session starts. User pastes the universal prompt from `AGENT_TEMPLATE.md`.
2. Worker scans `.queue/*.md`, parses front matter, finds first eligible task by ID order.
3. Worker edits the task file front matter: `status: wip`, commits this single edit to `master` via a tiny "claim" PR — **OR** simply edits the file in their worktree if they're trusted to be the only claimant. (We use the trusted model for now; race condition risk is low at our scale.)
4. Worker executes prompt body. Opens a feature branch in the specified worktree, does the work, opens PR.
5. Worker edits front matter: `status: pr_open`, `pr: <number>`.
6. Orchestrator (or user) merges the PR. After merge, orchestrator sets `status: done`.
7. Optional periodic compaction: orchestrator moves `done` tasks older than N days to `.queue/done/`.

## Parallel-safety reading

| Tasks                                                                 | Can run in parallel? |
| --------------------------------------------------------------------- | -------------------- |
| Two tasks both `status: todo` with disjoint `blocked_by` sets         | Yes                  |
| Two tasks where one's `merge_after` includes the other                | **Work yes, merge sequential** — worker uses mocks for not-yet-merged dependencies; PRs queue at merge time |
| Two tasks whose `worktree` paths collide                              | **No** — refuse to dispatch second |
| Two tasks whose owned-paths overlap (per spec §14.2-style ownership)  | **No** — refuse to dispatch second |

The orchestrator surfaces conflicts in `STATUS.md` if any auto-check trips.

## Orchestrator daily ritual

Once per UTC day, orchestrator runs:

1. `git fetch --all --prune`
2. For every `pr_open` task: check PR state. If merged → set `status: done`. If closed without merge → set `status: cancelled` with reason.
3. For every `wip` task: check that the branch has at least one commit. Stale `wip` (>7 days no commits) → flag in `STATUS.md`.
4. Rebase pending PRs on current `master` if they fall behind by >7 days.
5. Update `.queue/STATUS.md` with per-batch progress, blockers, and the next pickup hint.

## Conventions

- **Never** delete a task file in-place. Cancellation = `status: cancelled` + `notes` explaining why. Archival = move to `.queue/done/` (preserves git history of the rename).
- **Never** edit the prompt body of a task that has `status >= wip`. Spec-changes mid-flight create merge-hell. Open a successor task instead.
- **Coordination-owned files** (`Cargo.toml`, `src/lib.rs`, `src/main.rs`, `src/cli/mod.rs`, `src/runtime/mod.rs`, `src/vis/mod.rs`, `README.md`, `CHANGELOG.md`, `ROADMAP.md`, `TODO.md`) are NEVER modified by worker PRs. They go through a separate coordination PR opened daily by the orchestrator. Workers request module-registrations and Cargo deps in their PR body; orchestrator batches them.

## Where the rules come from

This queue formalises:

- `docs/UNIFIED_CHAT.md` §14 (coordination protocol)
- `docs/UNIFIED_CHAT_DECISIONS.md` (resolved spec questions)
- Project memory file `feedback_prompts_for_kimi.md` (prompt style for Kimi-k2.6 executors)

If any rule conflicts with the spec, the spec wins and this file gets corrected.
