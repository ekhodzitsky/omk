---
schema_version: 1
module: runtime::goal
level: subsystem
purpose: Proof-driven goal lifecycle — plan, execute, verify, deliver
status: pilot
surface:
  - name: GoalState
    kind: struct
    visibility: pub
    contract: Central state machine for a single goal. Owned by state/; all other modules treat it as pure data.
    proof:
      kind: unit-test
      target: runtime::goal::state
      command: cargo test --lib runtime::goal::state
  - name: GoalTaskGraph
    kind: struct
    visibility: pub
    contract: Dependency graph of tasks within a goal.
    proof:
      kind: unit-test
      target: runtime::goal::task_graph
      command: cargo test --lib runtime::goal::task_graph
  - name: GoalProof
    kind: struct
    visibility: pub
    contract: Immutable proof artifact of goal completion.
    proof:
      kind: integration-test
      target: goal_test
      command: cargo test --test goal_test
  - name: create_goal
    kind: fn
    visibility: pub
    contract: Create a new goal with scaffold (planner + task graph + initial state).
    proof:
      kind: integration-test
      target: goal_test
      command: cargo test --test goal_test
  - name: plan_goal
    kind: fn
    visibility: pub
    contract: Plan-only variant of create_goal (until_ready = false).
    proof:
      kind: integration-test
      target: goal_test
      command: cargo test --test goal_test
  - name: list_goals
    kind: fn
    visibility: pub
    contract: List all goals from state directory. Query function; lives in queries.rs.
    proof:
      kind: integration-test
      target: goal_test
      command: cargo test --test goal_test
  - name: resolve_goal
    kind: fn
    visibility: pub
    contract: Resolve a goal by id or "latest". Query function; lives in queries.rs.
    proof:
      kind: integration-test
      target: goal_test
      command: cargo test --test goal_test
  - name: resolve_goal_proof
    kind: fn
    visibility: pub
    contract: Resolve proof for a goal, with recovery fallback if proof file is missing/corrupt.
    proof:
      kind: integration-test
      target: goal_test
      command: cargo test --test goal_test
  - name: execute_goal
    kind: fn
    visibility: pub
    contract: Execute a goal lifecycle (orchestrate → dispatch → evidence).
    proof:
      kind: integration-test
      target: goal_test
      command: cargo test --test goal_test
  - name: verify_goal
    kind: fn
    visibility: pub
    contract: Verify goal completion criteria (local + agent tasks).
    proof:
      kind: integration-test
      target: goal_test
      command: cargo test --test goal_test
  - name: review_goal
    kind: fn
    visibility: pub
    contract: Review goal output and generate review artifacts.
    proof:
      kind: integration-test
      target: goal_test
      command: cargo test --test goal_test
  - name: cancel_goal
    kind: fn
    visibility: pub
    contract: Cancel a running goal.
    proof:
      kind: integration-test
      target: goal_test
      command: cargo test --test goal_test
  - name: pause_goal
    kind: fn
    visibility: pub
    contract: Pause a running goal.
    proof:
      kind: integration-test
      target: goal_test
      command: cargo test --test goal_test
  - name: resume_goal
    kind: fn
    visibility: pub
    contract: Resume a paused goal.
    proof:
      kind: integration-test
      target: goal_test
      command: cargo test --test goal_test
  - name: GoalStateStore
    kind: trait
    visibility: pub
    contract: Storage backend contract for GoalState. Isolates I/O for testability.
    proof:
      kind: unit-test
      target: runtime::goal::queries::tests
      command: cargo test --lib runtime::goal::queries
  - name: FileSystemGoalStateStore
    kind: struct
    visibility: pub
    contract: Production implementation using atomic file writes and tokio::fs.
    proof:
      kind: integration-test
      target: goal_recovery_test
      command: cargo test --test goal_recovery_test
  - name: InMemoryGoalStateStore
    kind: struct
    visibility: pub
    contract: In-memory mock for deterministic unit tests.
    proof:
      kind: unit-test
      target: runtime::goal::queries::tests
      command: cargo test --lib runtime::goal::queries
  - name: GoalGithubPrClient
    kind: trait
    visibility: pub
    contract: Trait for GitHub PR operations (create, update, merge) used by goal delivery. Enables testable PR delivery with mock implementations.
    proof:
      kind: unit-test
      target: runtime::goal::delivery
      command: cargo test --lib runtime::goal::delivery
  - name: GoalGithubPrCommandClient
    kind: struct
    visibility: pub
    contract: Production implementation of GoalGithubPrClient using the gh CLI command with a configurable timeout.
    proof:
      kind: unit-test
      target: runtime::goal::delivery
      command: cargo test --lib runtime::goal::delivery
  - name: GoalDeliveryPolicy
    kind: enum
    visibility: pub
    contract: Determines how a completed goal is delivered (local, draft-pr, or auto-pr).
    proof:
      kind: unit-test
      target: runtime::goal::delivery
      command: cargo test --lib runtime::goal::delivery
  - name: GoalAgentTaskProposal
    kind: struct
    visibility: pub
    contract: Proposed agent task with policy validation for a goal slice.
    proof:
      kind: integration-test
      target: goal_test
      command: cargo test --test goal_test
  - name: GoalGitEvidence
    kind: struct
    visibility: pub
    contract: Git evidence snapshot captured during goal execution (changed files, diff stats).
    proof:
      kind: integration-test
      target: goal_test
      command: cargo test --test goal_test
  - name: GoalOpenPrDraft
    kind: struct
    visibility: pub
    contract: Rendered PR draft (title, body, head branch) ready for GitHub delivery.
    proof:
      kind: integration-test
      target: goal_test
      command: cargo test --test goal_test
  - name: GoalKind
    kind: enum
    visibility: pub
    contract: Classification of the goal type (feature, fix, refactor, docs, etc.) used for planning and verification.
    proof:
      kind: integration-test
      target: goal_test
      command: cargo test --test goal_test
  - name: GoalProgressSnapshot
    kind: struct
    visibility: pub
    contract: Point-in-time progress summary with lines and current phase for HUD rendering.
    proof:
      kind: integration-test
      target: goal_test
      command: cargo test --test goal_test
  - name: GoalReplay
    kind: struct
    visibility: pub
    contract: Reconstructable replay log of a goal execution for debugging.
    proof:
      kind: integration-test
      target: goal_test
      command: cargo test --test goal_test
  - name: CreateGoalOptions
    kind: struct
    visibility: pub
    contract: Options bag for create_goal (budgets, delivery policy, until_ready flag).
    proof:
      kind: integration-test
      target: goal_test
      command: cargo test --test goal_test
  - name: GoalBudget
    kind: struct
    visibility: pub
    contract: Budget limits (time, tokens, USD) attached to a goal.
    proof:
      kind: integration-test
      target: goal_test
      command: cargo test --test goal_test
  - name: GoalId
    kind: struct
    visibility: pub
    contract: Unique goal identifier.
    proof:
      kind: integration-test
      target: goal_test
      command: cargo test --test goal_test
dependencies:
  internal:
    - module: runtime::atomic
      scope: state::persistence only
      reason: GoalState::save uses atomic file writes.
    - module: runtime::config
      scope: state directory paths
      reason: goals_dir() resolves canonical state path via runtime::config.
    - module: runtime::events
      scope: lifecycle and dispatch events
      reason: Goal execution emits structured event stream.
    - module: runtime::scheduler
      scope: task execution
      reason: "dispatch:: tasks are scheduled via TeamRunner."
    - module: runtime::worker
      scope: agent worker specs
      reason: dispatch prepares WorkerSpec for agent waves.
    - module: runtime::wire_worker
      scope: agent wire adapters
      reason: WireWorkerAdapter connects agent workers to Kimi CLI.
  external: []
consumers:
  - path: cli/goal/commands/mod.rs
    uses: [all public surface]
invariants:
  - id: queries-isolated
    rule: list_goals/resolve_goal/resolve_goal_proof are query functions; they live in queries.rs.
    proof:
      kind: static-check
      target: src/runtime/goal/queries.rs
      command: "test -f src/runtime/goal/queries.rs"
  - id: state-io-isolated
    rule: All GoalState I/O goes through GoalStateStore trait; GoalState is pure data.
    proof:
      kind: static-check
      target: src/runtime/goal/state/persistence.rs
      command: "! grep -q 'impl GoalState' src/runtime/goal/state/persistence.rs"
  - id: no-proxy-exports
    rule: dispatch/ modules do not proxy re-export parent items.
    proof:
      kind: static-check
      target: src/runtime/goal/dispatch/mod.rs
      command: "test $(wc -l < src/runtime/goal/dispatch/mod.rs) -le 10"
  - id: queries-testable
    rule: queries.rs functions have _with_store variants for in-memory testing.
    proof:
      kind: unit-test
      target: runtime::goal::queries::tests
      command: cargo test --lib runtime::goal::queries
verification:
  pre_change:
    - cargo test --lib runtime::goal
  full:
    - cargo test --test goal_test
    - cargo clippy --all-targets --all-features -- -D warnings
---

# runtime::goal

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                         cli/goal                            │
│                  (commands, arg parsing)                    │
└─────────────────────────┬───────────────────────────────────┘
                          │
┌─────────────────────────▼───────────────────────────────────┐
│                      runtime::goal                          │
│  ┌─────────┐  ┌──────────┐  ┌─────────┐  ┌──────────────┐ │
│  │ queries │  │ planner  │  │ lifecycle│  │   dispatch   │ │
│  │  (I/O)  │  │(scaffold)│  │(orchestrate)│ │(agent waves) │ │
│  └────┬────┘  └────┬─────┘  └────┬────┘  └──────┬───────┘ │
│       │            │             │               │         │
│  ┌────▼────────────▼─────────────▼───────────────▼──────┐  │
│  │                  state / task_graph                  │  │
│  │              (GoalState, GoalTaskGraph)              │  │
│  └──────────────────────────────────────────────────────┘  │
│       │            │             │               │         │
│  ┌────▼────┐  ┌────▼─────┐  ┌────▼────┐  ┌──────▼───────┐ │
│  │  proof  │  │ evidence │  │  budget  │  │    agent     │ │
│  │(artifacts)│ │(git/diff)│  │(limits)  │  │(policy/proposals)│ │
│  └─────────┘  └──────────┘  └─────────┘  └──────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

## Files

| File / Dir | Owns |
| --- | --- |
| `mod.rs` | Module exports, facade functions (`create_goal`, `plan_goal`). |
| `queries.rs` | Query functions: `list_goals`, `resolve_goal`, `resolve_goal_proof`. |
| `planner.rs` | `create_goal_with_scaffold` — builds initial goal + task graph. |
| `lifecycle.rs` | Orchestrates execute → verify → review pipeline. |
| `state/` | `GoalState` struct, `GoalStateStore` trait, persistence, constants. |
| `task_graph/` | `GoalTaskGraph`, delivery records, slice planning. |
| `dispatch/` | Agent task wave execution. `mod.rs` and `tasks/mod.rs` are storefronts. |
| `dispatch/tasks/` | Task payload builders, scheduler, results reader, wave runner. |
| `agent.rs` | Task policy, proposals, validation. |
| `budget.rs` | Budget tracking and evaluation. |
| `control.rs` | Pause/resume/cancel operations. |
| `decision.rs` | Goal decision tracking and acceptance criteria. |
| `delivery/` | PR delivery subsystem (`GoalGithubPrClient`, `GoalDeliveryPolicy`, `gh` command client). |
| `evidence.rs` | Git evidence detection, mutation snapshots. |
| `integration.rs` | Goal acceptance/rejection integration with external systems. |
| `open_pr.rs` | Render PR draft (title, body, branch) for GitHub delivery. |
| `oracle.rs` | Goal kind detection. |
| `progress.rs` | Progress line tracking. |
| `proof/` | Proof artifact helpers: review, sidecar, status. |
| `replay.rs` | Goal replay for debugging. |
| `verifier/` | Completion verification subsystem (local, review, security, tasks). |
| `worktree.rs` | Worktree planning and conflict detection. |
| `types.rs` | Shared types (`GoalId`, `GoalBudget`). |

## Edit Rules

- `mod.rs` is a storefront. Keep it under 100 lines.
- Do not add proxy re-exports in `dispatch/mod.rs` or `tasks/mod.rs`.
- Import directly from source modules using `crate::runtime::goal::*` paths.
- Keep I/O in `state/persistence.rs` and `queries.rs` only.
- Any new goal state field needs a migration note in `TODO.md`.

## Tests

```bash
cargo test --lib runtime::goal
cargo test --test goal_test
```
