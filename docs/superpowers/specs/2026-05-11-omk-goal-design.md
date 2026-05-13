# `omk goal` Design

Date: 2026-05-11

Status: design approved; early scaffold implemented with state, planning
artifacts, gates, git evidence, a policy-validated multi-task Wire-backed
execution wave with mutation evidence, accepted and later-dispatched
agent-proposed follow-up tasks, post-mutation gate reruns, and controller
review/security evidence.

Related docs:

- `SPEC.md`
- `ROADMAP.md`
- `TODO.md`
- `docs/COMPETITIVE_POSITIONING.md`
- `docs/ARCHITECTURE.md`
- `docs/PROJECT_MAP.md`

## Summary

`omk goal` is the main product direction for OMK. It is a long-running,
proof-driven engineering runtime that takes a high-level goal, decomposes it,
launches agents under policy, verifies the results, and exits only with a
truthful terminal status.

The feature exists to make "I am too lazy to manually run the whole engineering
process" a safe product primitive. Laziness is acceptable; fake completion is
not.

## Problem

Current AI coding tools can generate code quickly, but they often fail at the
parts that make engineering trustworthy:

- preserving behavior during rewrites;
- defining success criteria before implementation;
- coordinating multiple workers without write conflicts;
- continuing safely for hours or days;
- recovering after crashes;
- distinguishing "done" from "looks done";
- producing evidence that a human can inspect.

OMK already has useful primitives: Wire workers, scheduler-backed team runs,
events, gates, run inspection, and proof reports. `omk goal` should compose
those primitives into the top-level workflow.

Competitive boundary: `omk goal` is a local, repo-native, proof-driven
engineering runtime. It should be benchmarked against Devin, OpenHands, Claude
Code, Aider, Dify, and Cody, but it should not be described as a clone of any of
them.

## User Promise

The user can run:

```bash
omk goal run "Rewrite this Python service in Rust" --until-ready --budget-time 7d --budget-tokens 2000000 --budget-usd 25
```

and return later to a reliable answer:

- `ready`, with proof;
- `not_ready`, with failing evidence;
- `blocked_on_human`, with the exact decision needed;
- `blocked_on_external`, with missing access or dependency;
- `needs_more_budget`, with progress and remaining work; current code enforces exhausted wall-clock `--budget-time`, Wire-derived `--budget-tokens`, and estimated `--budget-usd` before more verify/execute/review work is spent and supports `budget-add` recovery for time, tokens, and USD;
- `paused`, with active Wire workers interrupted and durable progress preserved
  for later resume;
- `failed_infra`, with recovery guidance.

## Non-Goals

- Guarantee production-ready output for arbitrary underspecified ideas.
- Spawn unbounded recursive agents.
- Hide uncertainty behind confident summaries.
- Provision paid infrastructure or credentials without explicit user action.
- Replace product validation with synthetic claims.
- Rewrite large projects without first building a compatibility oracle.

## Design Principles

1. **Oracle first.** Define how readiness will be tested before executing.
2. **Proof is the product.** The final answer is only as good as the evidence.
3. **Agents propose; controller disposes.** Agents can suggest work, but the
   goal controller validates task graph mutations.
4. **Budgets are policy.** Time, cost, concurrency, files, commands, and
   external access are bounded.
5. **Long-running means resumable.** Goal runs must survive restart and context
   loss.
6. **Human decisions are first-class.** Blocking on a human is a valid outcome.
7. **Small merges beat giant diffs.** Accept subgoals through proof-backed
   increments.

## System Components

### Goal Controller

Owns the goal lifecycle.

Responsibilities:

- create goal state;
- classify the goal;
- produce initial artifacts;
- maintain the task graph;
- launch execution waves;
- enforce budgets and policies;
- decide terminal status;
- write final proof or failure artifact.

### Goal State Store

Persists durable state under `.omk/goals/<goal-id>/`.

Required files:

```text
goal.json
prd.md
technical-plan.md
test-spec.md
task-graph.json
decisions.jsonl
events.jsonl
artifacts/
reviews/
proof.json
failure.json
```

### Planner

Creates a PRD or goal brief, technical plan, test spec, task graph, and
controller decision log.

For greenfield goals, it defines acceptance tests and demo flows.

For rewrite goals, it defines compatibility surfaces and reference behavior.

For security goals, it defines threat model and audit gates.

For performance goals, it defines baseline and regression gates.

### Task Graph

The task graph is a DAG. Each task has:

- id;
- title;
- description;
- role;
- dependencies;
- read set;
- write set;
- risk;
- budget;
- acceptance criteria;
- status;
- owner;
- retry policy;
- evidence links.

### Agent Runtime

Uses existing OMK execution primitives:

- scheduler;
- Wire workers;
- role packs;
- event logs;
- verification gates;
- proof generation.

Agents can request new tasks or subagents, but the controller must approve the
change against policy.

Current slice: `goal-agent-execute` is internally expanded into bounded
controller-proposed tasks with worker-enforced per-task budgets. The controller writes
`task-policy.json`, emits `task_proposed`, `task_accepted`, and
`task_rejected`, and keeps external publishing disabled for the GitHub-only
release lane. Workers can also return structured `OMK_TASK_PROPOSAL: {...}`
follow-up work; the controller writes `agent-task-proposals.json`, applies the
same validation, appends accepted safe proposals as pending graph nodes, and
emits `task_graph_mutated` events with the source proposal artifact and
resulting task count. Agent-proposed follow-ups that share a write path,
normalized alias path, parent/child path, or read/write-overlapping path must be
dependency-ordered; unordered access conflicts are rejected before they can
mutate the durable task graph. Later `execute` invocations dispatch ready
pending follow-ups through a separate `goal-agent-followups` Wire wave and close
those durable graph nodes from worker results. Both built-in and follow-up agent
waves honor the goal `max_agents` policy by creating no more Wire workers than
the accepted ready task count or the configured cap. If a scheduler lease expires,
the controller emits
`retry_scheduled` evidence with the stale worker id and prefers another
available worker for the recovered task before falling back to the stale owner.
Recovered stale workers are quarantined with `worker_dead` evidence and a
durable `stale-worker-cleanup.json` marker; later stale-worker outbox and
heartbeat updates are ignored so recovered tasks cannot be overwritten by late
results.
When an operator pauses or cancels a goal during a Wire-backed wave, the active
execute process observes durable goal state, cancels workers, prevents any
additional scheduler dispatch, and preserves the interrupted goal/proof status.
Replay output is deterministic across separate CLI invocations: `generated_at`
is derived from persisted event evidence rather than the current process clock.
Task graphs are validated on load for duplicate ids, missing dependencies,
self-dependencies, required field presence, and dependency cycles before the
controller executes them.

### Verification Wall

Runs required gates and stores evidence.

Default Rust gates:

```bash
cargo fmt -- --check
cargo check --all-targets
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo doc --no-deps
```

Goal-specific gates:

- rewrite: golden and compatibility tests;
- greenfield: acceptance and smoke tests;
- security: dependency audit, threat model, secret scan;
- performance: benchmarks;
- frontend: browser QA and screenshots.

### Proof Generator

Consumes goal state, events, gates, reviews, and artifacts.

`ready` requires:

- all required gates passed;
- no unresolved critical review findings;
- no blocked required tasks;
- known gaps are non-critical or explicitly accepted;
- terminal criteria are satisfied.

## Data Flow

```text
User goal
  -> Goal intake
  -> Planning artifacts
  -> Oracle definition
  -> Task graph
  -> Agent execution waves
  -> Integration
  -> Verification wall
  -> Review loops
  -> Proof or failure
```

## Command Surface

MVP:

```bash
omk goal run <goal> [--until-ready] [--budget-time <duration>] [--budget-tokens <n>] [--budget-usd <usd>] [--max-agents <n>]
omk goal status [goal-id|latest]
omk goal show [goal-id|latest] [--format text|json|md]
omk goal list
omk goal cancel [goal-id|latest]
omk goal proof [goal-id|latest]
omk goal verify [goal-id|latest]
omk goal execute [goal-id|latest]
omk goal review [goal-id|latest]
omk goal pause [goal-id|latest]
omk goal resume [goal-id|latest]
omk goal replay [goal-id|latest] [--format text|json|md]
omk goal budget [goal-id|latest] [--format text|json|md]
omk goal budget-add [goal-id|latest] [--time <duration>] [--tokens <n>] [--usd <usd>]
```

Later:

```bash
omk goal plan <goal>
omk goal approve-plan <goal-id>
omk goal open-pr <goal-id>
```

## Goal Kinds

### Greenfield

Input: product or engineering idea.

Oracle:

- PRD acceptance criteria;
- generated tests;
- demo script;
- docs install flow;
- optional deploy smoke.

### Rewrite

Input: existing project and target language/architecture.

Oracle:

- old implementation as reference;
- golden fixtures;
- CLI/API compatibility tests;
- explicit incompatibility list.

### Refactor

Input: existing code quality or architecture goal.

Oracle:

- behavior tests stay green;
- targeted metrics improve;
- no public API break without changelog.

### Security Hardening

Input: audit or hardening objective.

Oracle:

- threat model;
- dependency audit;
- secret scan;
- abuse-case tests;
- review findings resolved or documented.

### Performance

Input: performance target.

Oracle:

- baseline;
- benchmark suite;
- regression thresholds.

## Policy Model

Every goal has policy:

- max agents;
- max budget time;
- max cost;
- allowed commands;
- denied commands;
- allowed write roots;
- whether network research is allowed;
- whether GitHub writes are allowed;
- whether destructive operations require human confirmation.

Agents cannot override policy.

## Human Gates

The controller must stop with `blocked_on_human` when:

- success criteria are ambiguous;
- product tradeoffs materially branch the solution;
- secrets or credentials are required;
- destructive actions are needed;
- dependency or license decisions are high risk;
- generated behavior intentionally differs from the old system.

## MVP Implementation Plan

1. Add goal state models and persistence.
2. Add CLI status/show/list/cancel.
3. Add planning artifact generation.
4. Add task graph schema.
5. Add a single-controller execution loop using existing `team run` primitives.
6. Add goal proof generation.
7. Add small greenfield and rewrite fixtures.
8. Add CI tests for terminal statuses.

## Acceptance Tests for MVP

- Creating a goal writes valid `goal.json`.
- Cancelling a goal writes `failure.json`.
- A goal with missing oracle stops as `blocked_on_human`.
- A fixture goal with passing gates ends as `ready`.
- A fixture goal with failing gates ends as `not_ready`.
- `goal show --format json` is stable and parseable.
- Resume after process restart continues from persisted state.

## Open Questions

These should be resolved during implementation planning:

1. Should the first MVP run real agents or only generate the goal plan and task
   graph?
2. Should worktrees be required for all execution, or only for parallel slices?
3. Should GitHub PR creation be part of MVP or stage 2?
4. What is the default max runtime for `--until-ready` without an explicit
   budget?
5. Should network research default to on or require a flag?

## Design Review

Placeholder scan: no placeholders remain.

Consistency check: the root spec, roadmap, and TODO all treat `omk goal` as the
top-level feature and reuse current OMK primitives.

Scope check: MVP is intentionally smaller than the full north star. It creates a
durable goal controller before attempting multi-day autonomous rewrites.

Ambiguity check: terminal statuses and readiness conditions are explicit enough
for an implementation plan.
