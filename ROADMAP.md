# OMK Roadmap

This roadmap tracks the path from the current Wire-first beta MVP to the
`omk goal` autonomous engineering runtime.

## North Star

```bash
omk goal run "Build or transform this project until it is proof-backed ready" --until-ready
```

The system should plan, research, spawn agents, assign tasks, verify results,
recover from failures, and stop only with a proof-backed terminal status.

Positioning is locked in `docs/COMPETITIVE_POSITIONING.md`: OMK is a local,
repo-native, proof-driven autonomous software engineering runtime, not a hosted
agent clone, visual app builder, or IDE chat product.

## Stage 0 - Current Foundation

Status: current beta MVP.

- Kimi-native asset sync, install, doctor, rollback.
- Scheduler-backed `omk team run`.
- Wire worker runtime.
- Event logs.
- Proof and failure artifacts.
- Run/proof/HUD inspection.
- Verification gates.
- `omk goal` durable scaffold with planning artifacts, task graph, local
  verification task evidence, git evidence, policy-validated bounded
  Wire-backed agent waves, accepted follow-up tasks, `max_agents` worker-pool
  caps, stale-lease recovery evidence, load-time task graph validation,
  first-class graph mutation events, path-normalized dependency-ordered
  read/write access conflict policy for agent-proposed follow-ups, mutation
  diff/changed-file evidence, post-mutation gate reruns, controller
  review/security evidence, and not-ready proof.
- GitHub CI and coverage.

## Stage 1 - Goal State Core

Target: make goals durable and inspectable.

- Add `.omk/goals/<goal-id>/` state layout.
- Add `omk goal run/status/show/list/proof/replay/budget/verify/execute/review/pause/resume/cancel`.
- Persist normalized goal, constraints, budgets, and terminal criteria.
- Emit goal lifecycle events.
- Write `failure.json` for blocked or failed goals.
- Add JSON and Markdown output for `goal show`.

Exit criteria:

- A goal can be created, inspected, replayed, budget-checked, verified locally,
  cancelled, and resumed after process restart.
- State transitions have tests.

## Stage 2 - Planning and Oracle

Target: make goals testable before execution.

- Generate PRD or goal brief.
- Generate technical plan.
- Generate test spec.
- Build task graph with dependencies plus read/write sets.
- Define the oracle for completion.
- Block execution when the oracle is missing.

Exit criteria:

- Greenfield and rewrite fixture goals produce different oracle shapes.
- `blocked_on_human` is used when success criteria are ambiguous.

## Stage 3 - Agent Orchestration

Target: let the goal controller create and manage work.

- Launch role-specific agents through existing team/runtime surfaces.
- Land the first bounded `goal-agent-execute` wave on existing scheduler/Wire
  primitives.
- Capture project mutation diff and changed-file evidence from the agent wave.
- Rerun gates after agent mutations.
- Record controller review/security evidence after the bounded agent wave.
- Allow agents to propose tasks.
- Require controller validation before mutating the task graph.
- Track heartbeats, leases, retries, stale work, and task ownership.
- Support bounded concurrency and cost/time budgets.

Exit criteria:

- A goal can execute multiple dependent tasks.
- Failed tasks retry or produce explicit proof evidence.

## Stage 4 - Verification Wall

Target: make readiness proof-backed.

- Run configured gates by project type.
- Capture command evidence and artifacts.
- Add compatibility/golden gates for rewrite goals.
- Add security and dependency gates for hardening goals.
- Add benchmark gates for performance goals.

Exit criteria:

- `ready` cannot be emitted while required gates are failing.
- `not_ready` includes the failing evidence.

## Stage 5 - Worktree and Integration Flow

Target: make parallel work safe.

- Create isolated worktrees or branches for independent task slices.
- Merge accepted slices through an integrator task.
- Detect access conflicts before dispatch. Initial agent-proposed follow-up
  conflicts, including normalized, parent/child, and read/write path overlaps,
  are now rejected unless dependency ordering serializes the access.
- Support partial acceptance of completed subgoals.
- Preserve changelog and docs updates during integration.

Exit criteria:

- Two independent slices can run concurrently and integrate deterministically.
- Conflicting read/write access sets block dispatch or require a plan change.

## Stage 6 - Self-Review and Hardening

Target: move from useful automation to autonomous engineering quality.

- Add specialist reviewer, security, performance, and test-engineer loops.
- Add "break it" challenge passes.
- Add anti-slop cleanup pass.
- Add dependency rationale checks.
- Add threat-model artifact for security-sensitive goals.

Exit criteria:

- A goal proof records independent review results.
- Known gaps are explicit and cannot be hidden by a final summary.

## Stage 7 - GitHub Output

Target: turn long-running goals into reviewable delivery artifacts.

- Open a PR or draft PR from a goal result.
- Attach proof summary to PR body.
- Link changed files, gates, known gaps, and decisions.
- Support release-candidate output for GitHub-only releases.

Exit criteria:

- `omk goal open-pr latest` creates a reviewable PR with proof evidence.

## Stage 8 - Long-Horizon Reliability

Target: let goals run for days safely.

- Harden pause/resume across active worker interruption and machine restarts. Current runtime now persists pause/resume across separate CLI invocations and interrupts active Wire-backed goal workers when an operator pauses or cancels a goal.
- Harden goal replay into deterministic crash-recovery replay. Current replay JSON is stable across separate CLI invocations because replay timestamps come from persisted goal event evidence.
- Harden token/cost budget sources and hard stops beyond the current wall-clock `--budget-time` guard and `budget-add` recovery path.
- Add crash recovery tests.
- Add stale agent cleanup.
- Add operator notifications.

Exit criteria:

- A multi-hour fixture goal can survive process restart and continue.
- Operators can answer "what is it doing?" without reading raw logs.

## Not Yet

These are deliberately out of early scope:

- Guaranteed production-ready output for arbitrary underspecified ideas.
- Unbounded recursive agent spawning.
- Automatic paid API or infrastructure provisioning.
- Rewriting very large projects without first building compatibility oracles.
- Silent force-push or destructive repository operations.
