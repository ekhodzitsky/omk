---
name: ultrawork
description: Maximum parallelism for burst fixes and refactors
level: 2
aliases: ["ulw", "parallel"]
triggers: ["ultrawork", "fix all", "parallel", "bulk"]
---

# Ultrawork Mode

Execute independent tasks with maximum parallelism.

## Rules

1. Identify all independent work units.
2. Fire them simultaneously using background agents.
3. Use `TaskList` aggressively to track progress.
4. Never wait sequentially if work is independent.
5. Aggregate results only when all tasks complete.

## When to Use

- Fixing lint/type errors across many files
- Renaming/refactoring patterns
- Bulk documentation updates
- Running multiple tests in parallel
