# OMK Goal Product Spec

`omk goal` is the north-star feature for oh-my-kimi.

It turns OMK from a set of useful orchestration commands into a long-running,
proof-driven engineering runtime. The command accepts a high-level outcome,
builds an evidence-backed plan, launches agents and subagents under policy, and
keeps working until the goal is ready, blocked, or out of budget.

Canonical detailed design:
`docs/superpowers/specs/2026-05-11-omk-goal-design.md`

## Product Thesis

Progress is powered by laziness: users should be able to express intent once and
let OMK do the tedious engineering work.

The product promise is not "generate lots of code." The product promise is:

> Work autonomously until the requested engineering goal is proof-backed ready,
> or produce a precise, actionable reason why it is not ready.

`omk goal` must be allowed to run for hours or days, but it must not be allowed
to claim success without evidence.

## Must-Have Positioning

Canonical market map: `docs/COMPETITIVE_POSITIONING.md`.

`omk goal` must be positioned as a local, repo-native, proof-driven autonomous
software engineering runtime. It is not a generic AI app builder, IDE
autocomplete product, hosted coding-agent clone, or unbounded recursive agent
launcher.

The direct competitive set is Devin, OpenHands, and Claude Code. Aider, Dify,
and Cody are adjacent benchmarks. OMK should learn from the category while
competing on durable goal state, explicit task graphs, verification gates, and
proof artifacts.

## Example Commands

```bash
omk goal run "Build a production-ready CLI for managing local LLM costs" --until-ready
omk goal run "Rewrite this Python project in Rust" --until-ready --budget-time 7d
omk goal status
omk goal show latest
omk goal pause latest
omk goal resume latest
omk goal cancel latest
```

## Scope

`omk goal` covers large engineering outcomes:

- greenfield products;
- rewrites and migrations;
- large refactors;
- bug-fix campaigns;
- security hardening;
- performance work;
- documentation and release readiness.

It does not replace human product judgment. When a goal depends on taste,
business strategy, credentials, paid APIs, or ambiguous acceptance criteria, the
correct outcome is `blocked_on_human`, not a fake success.

## Current Foundation

`omk goal` is planned, but it should reuse the current beta MVP instead of
inventing a parallel runtime:

- Kimi-native asset sync, doctor, install, and rollback;
- scheduler-backed `omk team run`;
- Wire worker control through `kimi --wire`;
- task claims, leases, retries, and write-set conflict checks;
- append-only event logs;
- verification gates;
- run/proof/HUD inspection;
- `proof.json` and `failure.json` artifacts.

The current foundation is documented in `README.md`, `docs/ARCHITECTURE.md`,
and `docs/PROJECT_MAP.md`.

## Core Outcomes

Every goal run ends in exactly one terminal status:

| Status | Meaning |
| --- | --- |
| `ready` | Required gates passed and the proof bundle supports the readiness claim. |
| `not_ready` | Work was attempted, but required proof or gates did not pass. |
| `blocked_on_human` | A human decision is required before progress can continue safely. |
| `blocked_on_external` | External access, credentials, APIs, or services are missing. |
| `needs_more_budget` | Time, token, cost, or compute budget was exhausted. |
| `failed_infra` | OMK infrastructure failed in a way the run could not recover from. |
| `cancelled` | User cancelled the goal. |

## Non-Negotiable Principles

1. **Proof over confidence.** Agents may propose completion; only verifiers and
   gates can accept completion.
2. **Oracle first.** Rewrites need compatibility tests. Greenfield work needs
   acceptance tests. Security/performance work needs explicit gates.
3. **Bounded autonomy.** Agents can request tasks and subagents, but the goal
   controller enforces policy, budgets, write scopes, and concurrency limits.
4. **No silent branching.** Material product or architecture choices are logged
   as decisions. Human-blocking decisions stop with `blocked_on_human`.
5. **Recoverable by default.** Goal state, task graph, messages, heartbeats,
   artifacts, and proofs must survive process crashes and context compaction.
6. **Small accepted increments.** Long goals are completed through accepted
   subgoals, not one giant unreviewable diff.
7. **Local-first.** OMK owns local state and execution; GitHub integration is an
   output surface, not the source of truth.

## Functional Requirements

### Goal Intake

- Accept a natural-language goal and optional constraints.
- Classify the goal as greenfield, rewrite, migration, refactor, audit, bugfix,
  performance, documentation, or mixed.
- Inspect the repository before planning.
- Create a goal directory under `.omk/goals/<goal-id>/`.
- Persist the original user request, normalized goal, assumptions, constraints,
  budgets, and terminal criteria.

### Planning

- Produce a PRD or goal brief.
- Produce a technical plan.
- Produce a test specification.
- Build a task graph with dependencies, read sets, write sets, risk level, and
  acceptance criteria for each task.
- Identify the oracle that will decide whether the goal is done.
- Stop early with `blocked_on_human` if the oracle cannot be defined.

### Agent Orchestration

- Launch role-specific agents and subagents through the existing OMK team/runtime
  surfaces.
- Assign bounded tasks with explicit ownership.
- Allow agents to propose follow-up tasks.
- Require the goal controller to approve task graph mutations.
- Track worker leases, heartbeats, retries, and failure evidence.
- Support long-running execution with resume after interruption.

### Research

- Search official docs and relevant repositories when the goal depends on
  libraries, frameworks, APIs, security practices, or migration strategies.
- Record sources and decisions in the goal decision log.
- Prefer official documentation and primary sources for implementation facts.

### Implementation

- Use isolated worktrees or branches for independent slices.
- Keep write scopes explicit.
- Merge accepted slices through an integrator step.
- Preserve changelog, docs, migration notes, and release notes as part of done.
- Avoid new dependencies unless a recorded decision justifies them.

### Verification

The verification wall is configurable, but the default Rust profile includes:

- `cargo fmt -- --check`
- `cargo check --all-targets`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all-features`
- `cargo doc --no-deps`
- dependency and license audit when configured

Additional gates are selected by goal type:

- rewrite: compatibility and golden tests against the original implementation;
- greenfield: acceptance, smoke, and demo tests;
- security: threat model, secret scan, dependency audit, abuse-case checks;
- performance: baseline and regression benchmarks;
- frontend: browser QA, responsive screenshots, accessibility checks.

### Proof Bundle

Each goal writes `.omk/goals/<goal-id>/proof.json` with:

- terminal status;
- goal summary;
- accepted and rejected assumptions;
- task graph summary;
- changed files;
- commits or branches produced;
- gates run and outputs;
- test results;
- reviews performed;
- security/performance notes;
- known gaps;
- human decisions required;
- links to artifacts.

### Commands

Initial command surface:

```bash
omk goal run <goal> [--until-ready] [--budget-time <duration>] [--max-agents <n>]
omk goal status [goal-id|latest]
omk goal show [goal-id|latest] [--format text|json|md]
omk goal list
omk goal pause [goal-id|latest]
omk goal resume [goal-id|latest]
omk goal cancel [goal-id|latest]
omk goal proof [goal-id|latest]
```

Later command surface:

```bash
omk goal plan <goal>
omk goal approve-plan <goal-id>
omk goal add-task <goal-id> <task>
omk goal open-pr <goal-id>
omk goal replay <goal-id>
```

## MVP Definition

The first usable `omk goal` MVP is not "rewrite any 200k line project."

It is:

- one long-running goal state directory;
- PRD, technical plan, and test spec artifacts;
- task graph persisted as JSON;
- limited agent execution through the existing team runner;
- status/resume/cancel;
- proof bundle;
- one greenfield demo;
- one rewrite/refactor demo using a small fixture;
- CI coverage for state transitions and proof statuses.

## State Layout

```text
.omk/goals/<goal-id>/
  goal.json
  prd.md
  technical-plan.md
  test-spec.md
  task-graph.json
  decisions.jsonl
  events.jsonl
  heartbeats/
  artifacts/
  reviews/
  proof.json
  failure.json
```

## Open Risks

- Agents can produce plausible but wrong code when the oracle is weak.
- Long-running goals can waste budget if task graph mutation is unconstrained.
- Parallel work can conflict without strong write-set enforcement.
- Security work needs explicit threat modeling, not only dependency scans.
- Product correctness cannot be fully automated without real-world feedback.

## Success Criteria

`omk goal` is successful when a user can start a large goal, leave the machine,
return later, and inspect a trustworthy state:

- what was attempted;
- what changed;
- what passed;
- what failed;
- what was not tested;
- what needs a human decision;
- whether the result is ready to merge or release.
