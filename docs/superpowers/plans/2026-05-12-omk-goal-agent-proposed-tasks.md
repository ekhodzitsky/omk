# `omk goal` Agent-Proposed Tasks Plan

**Goal:** Let Wire workers suggest follow-up work without letting them mutate
the durable goal task graph directly.

**Architecture:** Workers can include a structured line in their summary:

```text
OMK_TASK_PROPOSAL: {"id":"goal-agent-docs-followup", ...}
```

`omk goal execute` extracts those proposals after the bounded worker wave,
validates them through the same controller policy used for built-in task
proposals, writes `agent-task-proposals.json`, emits proposal/decision events,
and appends accepted safe work as pending `task-graph.json` nodes. Later
`execute` invocations dispatch ready pending follow-ups through the
`goal-agent-followups` Wire wave, bounded by the goal `max_agents` policy, and
close the graph nodes from worker results.

## Implementation Checklist

- [x] Add RED integration coverage for an agent-returned task proposal.
- [x] Extend the mock Wire fixture so tests can inject proposal text for a
      matching task prompt.
- [x] Parse `OMK_TASK_PROPOSAL` JSON objects from worker summaries.
- [x] Reject malformed, duplicate, unsafe-path, missing-dependency, zero-budget,
      and publishing proposals through controller policy.
- [x] Emit `task_proposed` from the worker actor and `task_accepted` /
      `task_rejected` from the controller actor.
- [x] Append accepted safe proposals to the goal task graph as pending nodes.
- [x] Dispatch accepted ready follow-up nodes on a later `goal execute`.
- [x] Enforce `max_agents` as a bounded worker pool for ready follow-up waves.
- [x] Update README, architecture docs, spec, TODO, changelog, and version.

## Verification

- `cargo test --test goal_cmd_test test_goal_execute_accepts_agent_proposed_task_graph_mutation`
- `cargo test --test goal_cmd_test test_goal_execute_dispatches_accepted_agent_followup_on_next_execute`
- `cargo test --test goal_cmd_test`

## Follow-Up

- Add stale-task recovery coverage for goal execution waves.
