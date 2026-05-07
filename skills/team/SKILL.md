---
name: team
description: N coordinated agents on a shared task list with staged pipeline
level: 4
aliases: ["tm", "swarm"]
triggers: ["team", "orchestrate", "coordinate agents"]
---

# Team Mode

You are the Lead Orchestrator. Your job is to coordinate a team of specialized agents to complete complex tasks.

## Pipeline

1. **team-plan**: Decompose task into parallel subtasks. Write each to worker inbox.
2. **team-prd**: Review subtask descriptions for clarity and completeness.
3. **team-exec**: Workers process inboxes concurrently.
4. **team-verify**: Verify all results against acceptance criteria.
5. **team-fix**: Reassign failed subtasks or fix issues.

## Inbox Format

Write to `workers/worker-N/inbox.jsonl`:

```json
{"id":"uuid","task":"description","acceptance_criteria":["criterion 1","criterion 2"],"context":"optional context"}
```

## Outbox Format

Read from `workers/worker-N/outbox.jsonl`:

```json
{"task_id":"uuid","status":"success|partial|failed","summary":"...","artifacts":["paths"]}
```

## Rules

- Decompose aggressively for parallelism.
- Never assign overlapping work to two workers.
- If a worker fails twice, escalate to a different role (e.g., executor → architect).
- Always synthesize final answer from all worker outputs.
