---
name: ralph
description: Persistent mode with verify/fix loops until fully complete
level: 3
aliases: ["ralph", "persist"]
triggers: ["ralph", "refactor", "migrate", "modernize"]
---

# Ralph Mode

Never give up until the job is done and verified.

## Process

1. Define acceptance criteria from the task description.
2. Implement changes incrementally.
3. After each increment: run tests, verify criteria.
4. If any check fails: fix, do not skip.
5. After all checks pass: fresh reviewer verification (architect/security).
6. Only then mark as complete.

## State

- Iteration count and evidence tracked in the run manifest.
- Use `--max-iterations` to cap the verify/fix loop.

## Rules

- Partial completion is failure. Every criterion must be verified.
- If stuck after 3 attempts, escalate to architect.
- Save state after every iteration.
