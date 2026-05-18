# Goal Troubleshooting and Manual Recovery

This guide covers manual recovery procedures for common `omk goal` delivery and merge failures. For general installation and runtime issues, see [`TROUBLESHOOTING.md`](TROUBLESHOOTING.md).

---

## Table of Contents

1. [Failed PR Creation](#failed-pr-creation)
2. [Failed CI Checks](#failed-ci-checks)
3. [Review Blockers](#review-blockers)
4. [Merge Conflicts](#merge-conflicts)
5. [Partial Acceptance](#partial-acceptance)
6. [Budget Exhaustion](#budget-exhaustion)

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
omk goal resume latest --delivery-policy pr
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

If the goal is genuinely too large, consider splitting it into smaller goals:

```bash
omk goal split latest --max-slices 3
```

---

## See Also

- [`TROUBLESHOOTING.md`](TROUBLESHOOTING.md) — general installation and runtime issues
- [`docs/TUTORIAL.md`](TUTORIAL.md) — getting started with `omk goal`
