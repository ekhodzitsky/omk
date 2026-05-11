# OMK Goal TODO

This is the implementation backlog for `omk goal`.

Canonical spec: `SPEC.md`
Detailed design: `docs/superpowers/specs/2026-05-11-omk-goal-design.md`

## Phase 1 - Durable Goal State

- [ ] Add `src/runtime/goal/` module.
- [ ] Define `GoalId`, `GoalStatus`, `GoalKind`, `GoalBudget`, `GoalState`.
- [ ] Define terminal statuses: `ready`, `not_ready`, `blocked_on_human`,
      `blocked_on_external`, `needs_more_budget`, `failed_infra`, `cancelled`.
- [ ] Add `.omk/goals/<goal-id>/` state path resolution.
- [ ] Persist `goal.json`.
- [ ] Persist `events.jsonl`.
- [ ] Write `failure.json` for non-ready terminal outcomes.
- [ ] Add state migration/version field.
- [ ] Add unit tests for serialization and backward-compatible reads.

## Phase 2 - CLI Surface

- [ ] Add `omk goal run <goal>`.
- [ ] Add `omk goal status [goal-id|latest]`.
- [ ] Add `omk goal show [goal-id|latest] --format text|json|md`.
- [ ] Add `omk goal list`.
- [ ] Add `omk goal cancel [goal-id|latest]`.
- [ ] Add `omk goal pause [goal-id|latest]`.
- [ ] Add `omk goal resume [goal-id|latest]`.
- [ ] Add command help smoke tests.
- [ ] Add JSON output smoke tests.

## Phase 3 - Planning Artifacts

- [ ] Generate `prd.md` or `goal-brief.md`.
- [ ] Generate `technical-plan.md`.
- [ ] Generate `test-spec.md`.
- [ ] Generate `task-graph.json`.
- [ ] Generate `decisions.jsonl`.
- [ ] Add a planning-only mode.
- [ ] Add `blocked_on_human` when success criteria cannot be made testable.
- [ ] Add tests for greenfield and rewrite planning fixtures.

## Phase 4 - Task Graph

- [ ] Define task node schema.
- [ ] Track dependencies.
- [ ] Track read sets and write sets.
- [ ] Track risk level.
- [ ] Track acceptance criteria.
- [ ] Track owner role.
- [ ] Track retries and lease expiration.
- [ ] Add graph validation.
- [ ] Add graph mutation events.
- [ ] Add tests for dependency ordering and write-set conflicts.

## Phase 5 - Agent Orchestration

- [ ] Implement goal controller loop.
- [ ] Reuse scheduler-backed team runner for execution tasks.
- [ ] Allow agents to propose new tasks.
- [ ] Validate proposed tasks against policy and budgets.
- [ ] Enforce max concurrency.
- [ ] Track heartbeats per worker.
- [ ] Recover stale tasks.
- [ ] Emit task accepted/rejected events.
- [ ] Add tests with mock agents.

## Phase 6 - Verification and Proof

- [ ] Add goal proof model.
- [ ] Capture gate command evidence.
- [ ] Capture changed files.
- [ ] Capture commits/branches.
- [ ] Capture review results.
- [ ] Capture known gaps.
- [ ] Block `ready` when required gates fail.
- [ ] Add `omk goal proof [goal-id|latest]`.
- [ ] Add golden proof tests.

## Phase 7 - Rewrite Oracle

- [ ] Detect source project command/API surfaces.
- [ ] Generate compatibility test plan.
- [ ] Add reference implementation runner.
- [ ] Add golden fixture capture.
- [ ] Compare outputs, errors, exit codes, and file artifacts.
- [ ] Track intentional incompatibilities.
- [ ] Add small Python-to-Rust fixture demo.

## Phase 8 - Greenfield Oracle

- [ ] Generate acceptance tests from PRD.
- [ ] Generate smoke/demo script.
- [ ] Generate docs-first usage examples.
- [ ] Add "engineering-ready" vs "product-ready" distinction.
- [ ] Add greenfield CLI/app fixture demo.

## Phase 9 - Worktree Integration

- [ ] Create isolated worktrees for independent slices.
- [ ] Track branch per task or subgoal.
- [ ] Add integrator step.
- [ ] Detect merge conflicts.
- [ ] Preserve changelog and docs updates.
- [ ] Add rollback for rejected slices.

## Phase 10 - Review Loops

- [ ] Add architect review pass.
- [ ] Add code review pass.
- [ ] Add test-engineer pass.
- [ ] Add security review pass.
- [ ] Add performance review pass.
- [ ] Add anti-slop cleanup pass.
- [ ] Persist review artifacts.

## Phase 11 - GitHub Delivery

- [ ] Add `omk goal open-pr`.
- [ ] Generate PR body from proof.
- [ ] Link artifacts and known gaps.
- [ ] Support draft PRs.
- [ ] Support release-candidate notes.

## Phase 12 - Long-Horizon Reliability

- [ ] Add pause/resume across process restart.
- [ ] Add crash recovery tests.
- [ ] Add budget checkpoints.
- [ ] Add notification hooks.
- [ ] Add stale worker cleanup.
- [ ] Add goal replay.

## Documentation Tasks

- [ ] Add `omk goal` tutorial after CLI MVP exists.
- [ ] Add troubleshooting entries for blocked goals.
- [ ] Add architecture diagram for goal controller and scheduler.
- [ ] Add examples for rewrite, greenfield, audit, and refactor goals.
- [ ] Refresh `docs/COMPETITIVE_POSITIONING.md` before each major `omk goal` release.
- [ ] Update README feature table when the first goal MVP lands.

## Release Gates for First Goal MVP

- [ ] `cargo fmt -- --check`
- [ ] `cargo check --all-targets`
- [ ] `cargo clippy --all-targets --all-features -- -D warnings`
- [ ] `cargo test --all-features`
- [ ] `cargo doc --no-deps`
- [ ] `cargo deny --all-features check advisories licenses`
- [ ] North Star goal fixture demo passes.
