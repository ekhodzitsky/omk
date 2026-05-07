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

1. Create `prd.json` with user stories and acceptance criteria.
2. Implement stories one by one.
3. After each story: run tests, verify criteria.
4. If any story fails: fix, do not skip.
5. After all stories pass: fresh reviewer verification (architect/critic).
6. Only then mark as complete.

## PRD Format

```json
{
  "user_stories": [
    {
      "id": "US-001",
      "description": "...",
      "acceptance_criteria": ["..."],
      "status": "not_started|in_progress|implemented|verified|failed"
    }
  ]
}
```

## Rules

- Partial completion is failure. Every story must be verified.
- If stuck after 3 attempts, escalate to architect.
- Save state after every iteration.
