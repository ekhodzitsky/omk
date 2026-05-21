---
id: 002
title: WS-06 — architect review pass with real rules
status: done
branch: ws/review-architect-real-rules
worktree: .worktrees/ws-review-architect-real-rules
blocked_by: [001]
merge_after: [001]
size: small
batch: audit-wave-1
pr: 112
notes: Merged 2026-05-21 as base for current master (SHA 8425033). First concrete consumer of F-02 ReviewPass trait.
---

# WS-06 — Architect ReviewPass

Implements first non-placeholder review pass in `src/runtime/goal/review/architect.rs`. Rules:

- File-size budget (configurable max_file_loc, default 400)
- Cross-module forbidden imports (configurable Vec, default empty)
- Registered via the F-02 `ReviewPass` trait

Added module declaration as a single line in `src/runtime/goal/review/mod.rs`. New file plus 4+ unit tests. Merged as PR #112 — current master HEAD (8425033) is this commit.

Pattern is now reusable for WS-07 (test-engineer), WS-08 (performance), and any future passes.
