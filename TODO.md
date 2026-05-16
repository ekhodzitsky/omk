# OMK Goal TODO

This is the implementation backlog for `omk goal`.

Canonical spec: `SPEC.md`
Detailed design: `docs/superpowers/specs/2026-05-11-omk-goal-design.md`
End-to-end delivery contract:
`docs/superpowers/specs/2026-05-14-omk-goal-end-to-end-delivery.md`

## Phase 1 - Durable Goal State

- [x] Add `src/runtime/goal/` module.
- [x] Define `GoalStatus` and `GoalState`.
- [x] Define dedicated `GoalId`, `GoalKind`, and `GoalBudget` types.
- [x] Define terminal statuses: `ready`, `not_ready`, `blocked_on_human`,
      `blocked_on_external`, `needs_more_budget`, `failed_infra`, `cancelled`.
- [x] Add `goals/<goal-id>/` path resolution under the OMK state directory.
- [x] Persist `goal.json`.
- [x] Persist `events.jsonl`.
- [x] Write `failure.json` for cancelled goals.
- [x] Add state version field.
- [x] Add unit tests for serialization.
- [x] Add backward-compatible read/migration tests.

## Phase 2 - CLI Surface

- [x] Add `omk goal run <goal>`.
- [x] Add `omk goal status [goal-id|latest]`.
- [x] Add `omk goal show [goal-id|latest] --format text|json|md`.
- [x] Add `omk goal list`.
- [x] Add `omk goal cancel [goal-id|latest]`.
- [x] Add `omk goal plan <goal>`.
- [x] Add `omk goal proof [goal-id|latest] --format text|json|md`.
- [x] Add `omk goal verify [goal-id|latest]`.
- [x] Add `omk goal execute [goal-id|latest]`.
- [x] Add `omk goal review [goal-id|latest]`.
- [x] Add `omk goal pause [goal-id|latest]`.
- [x] Add `omk goal resume [goal-id|latest]`.
- [x] Add command help smoke tests.
- [x] Add JSON output smoke tests.

## Phase 3 - Planning Artifacts

- [x] Generate `prd.md` or `goal-brief.md`.
- [x] Generate `technical-plan.md`.
- [x] Generate `test-spec.md`.
- [x] Generate `task-graph.json`.
- [x] Generate `decisions.jsonl`.
- [x] Add a planning-only mode.
- [x] Add `blocked_on_human` when success criteria cannot be made testable.
- [x] Add tests for greenfield and rewrite planning fixtures.

## Phase 4 - Task Graph

- [x] Define task node schema.
- [x] Track dependencies.
- [x] Track read sets and write sets.
- [x] Track risk level.
- [x] Track acceptance criteria.
- [x] Track owner role.
- [x] Track retries and lease expiration.
- [x] Add graph validation.
- [x] Add graph mutation events.
- [x] Add tests for dependency ordering and exact/normalized/parent-child/read-write access conflicts.

## Phase 5 - Agent Orchestration

- [x] Implement first policy-validating goal controller loop.
- [x] Add local controller execution step for verification task evidence.
- [x] Reuse scheduler-backed team runner for the first bounded execution task.
- [x] Capture mutation diff and changed-file evidence from the first execution wave.
- [x] Add controller-proposed multi-task dispatch for `goal-agent-execute`.
- [x] Allow agents to propose new tasks back to the controller.
- [x] Validate proposed tasks against policy and per-task budgets.
- [x] Dispatch accepted agent-proposed follow-up tasks on later `goal execute` invocations.
- [x] Enforce max concurrency.
- [x] Track heartbeat artifacts for the first goal worker wave.
- [x] Recover stale tasks.
- [x] Emit task proposed/accepted/rejected events.
- [x] Add tests with a mock Wire agent for the first execution wave.

## Phase 6 - Verification and Proof

- [x] Add goal proof model.
- [x] Capture gate command evidence.
- [x] Capture changed files.
- [x] Capture commits/branches.
- [x] Capture controller review results.
- [x] Rerun verification gates after agent mutations.
- [x] Capture known gaps.
- [x] Block `ready` when required gates fail.
- [x] Add `omk goal proof [goal-id|latest]`.
- [x] Add golden proof tests, including `ready`, `not_ready`,
      `blocked_on_human`, `needs_more_budget`, `cancelled`, and
      infra-like terminal status coverage.
- [x] Add ready-path greenfield and rewrite fixture tests with oracle,
      review, integration, and PR draft evidence.

## Phase 7 - Rewrite Oracle

- [x] Detect source project command/API surfaces.
- [x] Generate compatibility test plan.
- [x] Add reference implementation runner.
- [x] Add golden fixture capture.
- [x] Compare outputs, errors, exit codes, and file artifacts.
- [x] Track intentional incompatibilities.
- [x] Add small Python-to-Rust fixture demo.

## Phase 8 - Greenfield Oracle

- [x] Generate acceptance tests from PRD.
- [x] Generate smoke/demo script.
- [x] Generate docs-first usage examples.
- [x] Add "engineering-ready" vs "product-ready" distinction.
- [x] Add greenfield CLI/app fixture demo.

## Phase 9 - Worktree Integration

- [x] Treat `master` / `main` as read-only baselines in docs, PR templates,
      and future automation. (Docs and PR template done; CI/branch-protection
      automation tracked separately.)
- [x] Add goal task ownership for parallel goal task slices.
- [x] Create isolated worktrees for independent slices.
- [x] Track branch per task or subgoal.
- [x] Track task id and PR link per task or subgoal.
- [x] Add integrator step.
- [x] Detect merge conflicts and record task delivery evidence.
- [x] Preserve changelog and docs updates.
- [x] Add rollback for rejected slices.

## Phase 10 - Review Loops

- [x] Add initial controller review pass.
- [x] Add initial controller security evidence pass.
- [x] Persist initial review artifacts.
- [x] Add architect review pass.
- [x] Add code reviewer pass.
- [x] Add test-engineer pass.
- [x] Add specialist security review pass.
- [x] Add performance review pass.
- [x] Add anti-slop cleanup pass.

## Phase 11 - GitHub Delivery

- [x] Add `omk goal open-pr`.
- [x] Generate PR body from proof.
- [x] Include task id, owner, write scope, verification wall output, and known
      gaps in generated PR bodies.
- [x] Link artifacts and known gaps.
- [x] Support draft PR dry-run metadata.
- [x] Support release-candidate notes in PR dry-run output.

## Phase 12 - End-to-End Delivery Controller

- [x] Keep the primary UX one-command: `omk goal run ... --until-ready`;
      additional commands are for inspection, recovery, and explicit policy
      changes, not for driving the happy path step by step.
- [x] Keep the first operator surface TUI/terminal-first; defer graphical UI
      until the end-to-end terminal flow is reliable.
- [x] Add orchestrator narrative updates: what was implemented, what is being
      checked, what comes next, blockers, and material tradeoffs under
      consideration.
- [x] Add explicit delivery policy for automatic PR/merge side effects.
- [x] Auto-decompose large goals into PR-sized delivery slices.
- [x] Assign each slice an owner role, write scope, branch, worktree, gates,
      review requirements, and integration dependency.
- [x] Materialize task-scoped branches/worktrees from the goal controller.
- [x] Dispatch agents per slice through the existing scheduler/Wire runtime.
- [x] Commit accepted slice changes with proof and task metadata.
- [x] Push task branches and create draft PRs when policy permits.
- [x] Convert review findings into fix tasks and repeat review/fix loops until
      blockers are resolved or the goal is blocked with evidence.
- [x] Create or update an integrator branch/PR that combines accepted slices.
- [x] Rerun verification gates on the integrator branch before opening the
      integrator PR.
- [x] Update `proof.json` with task PRs, commits, reviews, merge status,
      integrator PR, and final baseline commit (delivery metadata schema
      implemented; CI runs field reserved for future GitHub API integration).
- [x] Add an end-to-end fixture proving: plan -> worktree -> agent -> delivery
      metadata -> narrative TUI.

## Phase 12 Leftovers / Next Release

These are implemented at the scaffold/code level but need hardening or
real-world validation before they are considered fully closed:

- [ ] Treat routine polish as part of the goal: anti-slop review should
      automatically spawn cleanup/refactor follow-up tasks when evidence shows
      rough edges, not just record the finding in `proof.json`.
- [ ] Run the full 6-review wall (architect, code, test, security, performance,
      anti-slop) against each slice PR. Current per-slice review runs gates +
      security scan only.
- [ ] Detect integration conflicts with `git merge-tree`, rebase/update task PRs
      when safe, and record conflict evidence when automatic recovery is unsafe.
      Current merge-tree check exists but auto-rebase is not implemented.
- [ ] Gate final merge into `main` / `master` on proof, CI, review wall, and
      delivery policy. `merge_policy` (`gated`/`manual`/`disabled`) is wired but
      the actual merge action relies on `gh pr merge` polling and needs
      end-to-end validation with real GitHub PRs.
- [ ] Document manual recovery for failed PR creation, failed CI, review
      blockers, merge conflicts, and partial acceptance.
- [x] Concurrent slice execution with non-overlapping write scopes in isolated
      git worktrees. Overlapping scopes are still serialized.

## Phase 13 - Long-Horizon Reliability

- [x] Add pause/resume across process restart.
- [x] Harden pause/resume against active worker interruption.
- [x] Add crash recovery tests.
- [x] Add budget checkpoints.
- [x] Enforce exhausted wall-clock `--budget-time` before goal verify/execute/review.
- [x] Add operator recovery for `needs_more_budget` goals with `budget-add`.
- [x] Enforce per-task Wire worker budget hard stops.
- [x] Add token/cost budget sources and hard stops.
- [x] Document no-dependency notification hook extension point.
- [x] Add stale worker cleanup.
- [x] Add goal replay.
- [x] Harden goal replay into deterministic crash-recovery replay.

## Documentation Tasks

- [x] Add `omk goal` tutorial after CLI MVP exists.
- [x] Add troubleshooting entries for blocked goals.
- [x] Add architecture diagram for goal controller and scheduler.
- [x] Add examples for rewrite, greenfield, audit, and refactor goals.
- [x] Refresh `docs/COMPETITIVE_POSITIONING.md` before each major `omk goal` release.
- [x] Update README feature table when the first goal MVP lands.

## Release Gates for First Goal MVP

- [x] `cargo fmt -- --check`
- [x] `cargo check --all-targets`
- [x] `cargo clippy --all-targets --all-features -- -D warnings`
- [x] `cargo test --all-features`
- [x] `cargo doc --no-deps`
- [x] `cargo deny --all-features check advisories licenses`
- [x] North Star goal fixture demo passes.
