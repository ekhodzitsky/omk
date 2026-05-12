# `omk goal` Review/Security Evidence Plan

Date: 2026-05-12

## Goal

Make the post-agent readiness gap explicit in the goal task graph and add a
controller-owned review/security evidence pass without claiming full production
readiness yet.

## Scope

- Add explicit `goal-review` and `goal-security-review` tasks.
- Add `omk goal review [goal-id|latest]`.
- Persist review artifacts under `artifacts/reviews/`.
- Keep `proof.json` honest: `not_ready` remains until project mutation and
  integration loops exist.

## Test First

- [x] `goal --help` lists `review`.
- [x] `goal run` creates six tasks, with review/security pending.
- [x] `goal execute` closes local verify and agent execution, leaving
  review/security pending.
- [x] `goal review` closes review/security tasks and writes artifacts.
- [x] Proof moves past review/security gaps but still records the
  no-mutation/integration gap.

## Implementation

- [x] Add runtime constants for review/security task IDs and artifact names.
- [x] Extend initial task graph with review/security tasks.
- [x] Add `review_goal` runtime entrypoint.
- [x] Add controller review artifact generation.
- [x] Add bounded high-confidence changed-file secret scan.
- [x] Add CLI command and task status output.

## Verification

- [x] `cargo test --test goal_cmd_test`
- [x] Full release verification
- [ ] Commit, push, and watch CI
