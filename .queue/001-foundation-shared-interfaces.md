---
id: 001
title: Foundation — shared interfaces for parallel workstreams
status: done
branch: ws/foundation-shared-interfaces
worktree: (cleaned up after merge)
blocked_by: []
merge_after: []
size: medium
batch: parallel-foundation
pr: 109
notes: Merged 2026-05-21. Unlocks F-01..F-04 dependencies for downstream tasks.
---

# Foundation PR

This task seeded four shared interfaces touched by ≥2 downstream tasks, so parallel PRs would not conflict on them:

- **F-01:** `src/runtime/goal/task_graph/delivery/metadata.rs` — added `conflict_evidence_path`, `conflict_blocking_reason`, `slice_lease_id` (`Option<...>` with serde defaults).
- **F-02:** `src/runtime/goal/review/pass.rs` (new) — `ReviewPass` trait + `ReviewPassRegistry`. Six review-pass tasks (architect/code/test-engineer/security/perf/anti-slop) each only add a new file + 1 line in `review/mod.rs`.
- **F-03:** `src/runtime/db/migrations/runner.rs` (new) — `MigrationRunner` with upgrade chain, supersedes hard-coded `if version < 1 { INITIAL_SCHEMA }`.
- **F-04:** `.gitignore` — ignore sibling reference clones (`aider/`, `bernstein/`, `cline/`, `cli-agent-orchestrator/`, `SWE-agent/`).

PR #109 merged cleanly with four atomic commits. Subsequent tasks reference these interfaces by name.
