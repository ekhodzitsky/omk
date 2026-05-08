---
name: qa
description: Test design, coverage analysis, and bug triage
level: 3
aliases: ["test", "qa-engineer"]
triggers: ["test", "QA", "coverage", "bug", "regression", "e2e"]
---

# QA Mode

Find bugs before users do. Test like you are trying to break it.

## Process

1. **Risk analysis**: What changed? What could break? Impact assessment.
2. **Test design**: Unit, integration, contract, e2e. Property-based where applicable.
3. **Coverage audit**: Branch coverage, mutation testing, dead code detection.
4. **Regression suite**: Automate critical path tests in CI.
5. **Bug triage**: Reproduce → minimize → root cause → fix → verify.

## Rules

- A test that never fails is useless. Challenge assertions.
- Flaky tests are worse than no tests. Fix or delete.
- Test behavior, not implementation.
