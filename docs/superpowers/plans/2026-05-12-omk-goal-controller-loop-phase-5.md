# `omk goal` Controller Loop Phase 5 Plan

**Goal:** Replace the single bounded `goal-agent-execute` scheduler task with a
policy-validated multi-task controller wave.

**Architecture:** Keep the current goal task graph stable: `goal-agent-execute`
remains the aggregate proof node. Inside that node, the controller proposes
bounded scheduler tasks, validates each proposal against local policy, assigns a
per-task budget plus acceptance criteria, emits proposal/decision events, writes
`task-policy.json`, and dispatches only accepted tasks through the existing
Wire-backed scheduler.

**Policy for this slice:**

- Accept bounded in-repository implementation and follow-up verification tasks.
- Reject external publish side effects; crates.io publishing stays disabled
  while releases are GitHub-only.
- Reject unsafe paths, duplicate task IDs, missing dependencies, and zero
  budgets.
- Keep the aggregate proof honest: this is still `not_ready` until integration
  and readiness loops exist.

## Implementation Checklist

- [x] Add RED integration test for multi-task dispatch, budgets, policy artifact,
      rejected publish task, and `task_proposed` / `task_accepted` /
      `task_rejected` events.
- [x] Add event kinds for task proposal and policy decisions.
- [x] Expand `goal-agent-execute` into controller-proposed scheduler tasks.
- [x] Persist `artifacts/agent-runs/goal-agent-execute/task-policy.json`.
- [x] Pass acceptance criteria and per-task budget context to Wire workers.
- [x] Record task policy evidence on the aggregate goal task.
- [x] Enforce `max_agents` as a bounded Wire worker pool for accepted ready
      tasks.
- [x] Recover stale task leases and prefer another worker for recovered work.
- [x] Validate task graphs on load for duplicate ids, missing dependencies, and
      dependency cycles.
- [x] Update README, architecture docs, spec, TODO, changelog, and version.

## Verification

- `cargo test --test goal_cmd_test test_goal_execute_dispatches_policy_validated_multi_task_agent_wave`
- `cargo test --test goal_cmd_test test_goal_execute_uses_max_agents_worker_pool_for_ready_followups`
- `cargo test --test goal_cmd_test test_goal_execute_recovers_stale_agent_task_on_another_worker`
- `cargo test runtime::goal::task_graph::tests::validate_`
- `cargo test --test goal_cmd_test`

## Completed Follow-Up

- [x] Persist first-class graph mutation events beyond the current task
      proposal/accept/reject event stream.
