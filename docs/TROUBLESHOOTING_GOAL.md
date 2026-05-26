# Goal Troubleshooting and Manual Recovery

This guide covers manual recovery procedures for common `omk goal` delivery and merge failures. For general installation and runtime issues, see [`TROUBLESHOOTING.md`](TROUBLESHOOTING.md).

---

## Table of Contents

1. [Stuck Goal / Stagnation](#stuck-goal--stagnation)
2. [Circuit Breaker Tripped](#circuit-breaker-tripped)
3. [Pool Exhaustion](#pool-exhaustion)
4. [Failed PR Creation](#failed-pr-creation)
5. [Failed CI Checks](#failed-ci-checks)
6. [Review Blockers](#review-blockers)
7. [Merge Conflicts](#merge-conflicts)
8. [Partial Acceptance](#partial-acceptance)
9. [Budget Exhaustion](#budget-exhaustion)

---

## Stuck Goal / Stagnation

### Symptom

The goal appears to be running but makes no meaningful progress. Iterations repeat the same fixes, proof score does not improve, or the agent cycles between failed gates:

```
stagnation detected: CircularFix (confidence 0.91)
controller blocked: agent stuck in retry loop
```

### Diagnostic

Run the stagnation detector explicitly:

```bash
omk goal diagnose latest
omk goal diagnose latest --json | jq '.diagnosis.primary, .confidence'
```

Inspect the metric history and checkpoint state:

```bash
omk goal show latest --format json | jq '.recovery_attempts, .phase'
omk goal proof latest --format md
```

### Recovery

**Generate a recovery plan** and review it before applying:

```bash
omk goal recover latest
```

If the plan looks correct, create a checkpoint and let the controller resume with the recovery strategy:

```bash
omk goal resume latest
```

**If recovery made things worse**, roll back to the last checkpoint:

```bash
omk goal rollback latest
omk goal resume latest
```

**If the goal is genuinely blocked by an external dependency**, reject it with a reason and start a new goal when the dependency is resolved:

```bash
omk goal reject latest --reason "blocked on upstream API change"
```

---

## Circuit Breaker Tripped

### Symptom

A verification gate fails repeatedly and the circuit breaker opens, causing the gate to be skipped entirely:

```
circuit breaker OPEN for gate "cargo test": skipping (5 failures)
proof remains not_ready because required gate was skipped
```

### Diagnostic

Check circuit breaker state:

```bash
omk gates status
omk gates status --json | jq '.breakers[] | select(.state == "Open")'
```

Inspect the gate failure evidence:

```bash
omk goal proof latest --format json | jq '.gates[] | select(.name == "cargo test")'
```

### Recovery

**Fix the underlying issue**, then reset the breaker:

```bash
# Fix the code locally on the slice branch
git checkout <slice-branch>
# edit, commit, push
git add -A && git commit -m "fix: address gate failure"
git push

# Reset the breaker after the fix
omk gates reset --gate "cargo test"
omk goal verify latest
omk goal resume latest
```

**If the gate is flaky and the code is correct**, reset the breaker and increase the threshold in `.gates.toml`:

```bash
omk gates reset --gate "cargo test"
```

Then edit `.gates.toml` to raise `failure_threshold` or `recovery_timeout_secs` for that gate.

**Reset all tripped breakers** (use with caution):

```bash
omk gates reset --all
```

---

## Pool Exhaustion

### Symptom

The scheduler cannot admit new tasks because all pool slots are occupied:

```
pool "default" full: 8/8 active tasks, 3 queued
controller blocked: max_workers exceeded
```

### Diagnostic

Check pool utilization:

```bash
omk pools status
omk pools status --json | jq '.pools[] | {name, active_tasks, queued_tasks, max_workers}'
```

List active goal tasks:

```bash
omk goal show latest --format json | jq '.task_graph.tasks[] | select(.status == "running")'
```

### Recovery

**Wait for running tasks to complete.** The queue is FIFO with priority override; tasks are automatically promoted when slots free.

**If a task is stuck or dead**, cancel it to free a slot:

```bash
omk goal pause latest
omk goal cancel latest   # frees all goal slots
# or resume after the stuck worker is cleaned up
omk goal resume latest
```

**Clean up stale queue entries** after a crash or abrupt termination:

```bash
omk pools cleanup
```

**If the pool limit is too low** for the current workload, raise `max_workers` in `~/.config/omk/config.toml` under the `[pools.default]` section and restart the goal.

---

## Failed PR Creation

### Symptom

`omk goal` reports a delivery failure or blocker such as:

```
gated merge blocked: PR merge failed: ...
slice branch merge check failed: ...
```

### Diagnostic

Inspect the goal state and proof artifact:

```bash
omk goal status latest
omk goal show latest --format json | jq '.delivery_policy, .merge_policy'
omk doctor                    # verify gh CLI auth and repo push access
```

Check the proof artifact for the exact reason:

```bash
omk goal proof latest --format md
```

### Recovery

**If the delivery policy is too restrictive**, retry with a different policy:

```bash
# Switch from DraftPr to Pr or Local
omk goal resume latest --policy pr
```

**If GitHub auth or push access is missing**, fix authentication and resume:

```bash
gh auth login
gh auth status
omk goal resume latest
```

**If the PR exists but OMK lost the URL**, record it manually and resume:

```bash
# Find the branch in the goal state
omk goal show latest --format json | jq '.artifacts[] | select(.kind | contains("delivery"))'

# Open or update the PR manually
gh pr create --head <slice-branch> --base main --title "[slice] ..." --body "..."
# or
gh pr view <slice-branch>

# Then resume the goal controller
omk goal resume latest
```

---

## Failed CI Checks

### Symptom

The integrator or slice PR passes local gates but fails on GitHub CI, or the gated merge times out waiting for CI:

```
gated merge blocked: required CI checks did not pass within timeout
```

### Diagnostic

Read the gate evidence and GitHub check status:

```bash
omk goal proof latest --format json | jq '.gates'
gh pr checks <pr-url>  # or open the PR in the browser
```

### Recovery

**Fix the code locally**, commit, and push:

```bash
git checkout <slice-branch>
# edit files
git add -A && git commit -m "fix: address CI failure"
git push
```

Then re-run local verification and resume the goal:

```bash
omk goal verify latest    # re-run gates locally
omk goal resume latest
```

**If CI is flaky and the code is correct**, you can bypass the gated merge with the `Manual` merge policy:

```bash
omk goal resume latest --merge-policy manual
# Then merge manually when CI recovers:
gh pr merge <pr-url> --squash --delete-branch
```

---

## Review Blockers

### Symptom

The review wall reports failures (security, architecture, code, test, performance, or anti-slop):

```
gated merge blocked: proof validation failed: review wall has failures or blockers
```

### Diagnostic

Inspect the review artifacts:

```bash
omk goal proof latest --format md
omk goal proof latest --format json | jq '.review_artifacts'
```

Look for `status: "blocked"` entries and their `known_gaps`.

### Recovery

**Address the findings locally** on the slice branch:

```bash
git checkout <slice-branch>
# apply fixes
git add -A && git commit -m "fix: address review findings"
git push
```

Then re-run the review:

```bash
omk goal review latest
omk goal verify latest
omk goal resume latest
```

**If a review category is not applicable**, you can accept the known gap with a reason:

```bash
omk goal accept latest --summary "accept architecture gap: change is purely mechanical refactoring"
```

---

## Merge Conflicts

### Symptom

The slice branch has merge conflicts with the base branch, and auto-rebase failed:

```
slice branch <branch> cannot merge cleanly into main and auto-rebase failed: ...
```

### Auto-Rebase Behavior

Before pushing, `omk goal` automatically attempts to rebase stale slice branches onto the current base branch. Auto-rebase succeeds when:

- The slice branch is behind the base (no divergent changes)
- The slice and base modified different files

Auto-rebase fails when:

- Both the slice and base modified the same file(s) in incompatible ways
- The slice branch has been force-pushed or rewritten

If auto-rebase fails, the delivery is blocked and you must resolve conflicts manually.

### Diagnostic

Check the conflict evidence in the proof artifact:

```bash
omk goal proof latest --format md
omk goal proof latest --format json | jq '.known_gaps'
```

### Recovery

**Manually rebase the slice branch** onto the latest base:

```bash
git fetch origin
git checkout <slice-branch>
git rebase origin/main
# resolve conflicts in each file
git add <conflicted-file>
git rebase --continue
```

If the rebase becomes too complex, abort and merge instead:

```bash
git rebase --abort
git merge origin/main
# resolve conflicts, then commit
git push
```

After resolution, verify locally and resume:

```bash
omk goal verify latest
omk goal resume latest
```

---

## Partial Acceptance

### Symptom

Some slices are ready and pass all checks, but others fail. You want to merge only the good slices.

### Diagnostic

List slice statuses:

```bash
omk goal show latest --format json | jq '.task_graph.slices'
omk goal proof latest --format json | jq '.task_graph_summary'
```

### Recovery

**Reject the failed slices individually**:

```bash
omk goal reject latest --reason "slice B failed security review, needs rewrite"
```

**Accept the good slices**:

```bash
omk goal accept latest --summary "accept slices A, C, D; reject B"
```

Then re-run the integrator with the remaining subset:

```bash
omk goal execute latest   # re-run integrator with remaining slices
```

---

## Budget Exhaustion

### Symptom

The goal stops with a budget or rate-limit blocker:

```
controller blocked: budget exhausted for this goal
```

### Diagnostic

Check the current budget and spend:

```bash
omk goal show latest --format json | jq '.budget, .spent'
```

### Recovery

Add budget to the goal and resume:

```bash
omk goal budget-add latest --tokens 500000 --usd 5.00
omk goal resume latest
```

If the goal is genuinely too large, consider splitting it into smaller goals
manually and starting new goal sessions for each part.

---

## See Also

- [`TROUBLESHOOTING.md`](TROUBLESHOOTING.md) — general installation and runtime issues
- [`docs/TUTORIAL.md`](TUTORIAL.md) — getting started with `omk goal`
