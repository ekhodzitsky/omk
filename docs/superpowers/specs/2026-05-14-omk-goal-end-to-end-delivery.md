# `omk goal` End-to-End Delivery Contract

Date: 2026-05-14

Status: product requirement. This describes the intended north-star behavior;
it is not fully implemented until the acceptance tests in this document pass.

Related docs:

- `SPEC.md`
- `TODO.md`
- `ROADMAP.md`
- `docs/superpowers/specs/2026-05-11-omk-goal-design.md`

## Product Requirement

`omk goal` must be able to run a large engineering goal end to end. "End to end"
means the controller does more than produce a local proof or a PR draft. Under
an explicit delivery policy, it can plan, decompose, create branches/worktrees,
assign agents, implement, review, fix, audit, refactor, create PRs, integrate
those PRs, and merge the accepted result into the protected baseline.

The user-facing promise is:

```text
Give OMK one engineering goal. It does the internal engineering process under
the hood and returns either merged, proof-backed repository changes or precise
blocker evidence.
```

## One-Command UX

The product is for lazy, high-leverage engineering. The happy path must not
become a dashboard of manual subcommands. The user should be able to run one
command, leave for tea, and return to a professional software delivery process
that is either still progressing, fully completed, or blocked with exact
evidence.

Additional commands may exist for inspection, pause/resume, policy changes, and
manual recovery, but they must not be required to drive the normal flow.
Internally, OMK can perform many steps; externally, the product promise is one
goal command plus trustworthy evidence.

## TUI-First Control Surface

The first product surface is terminal-native. A graphical interface is optional
later work; it must not block the first useful version.

The user should be able to install OMK quickly, run one goal command, and watch
a minimal textual control surface. The TUI should read like a professional
orchestrator narrating the work:

- "implemented X";
- "running verification Y";
- "review found Z, creating fix task";
- "considering approach A vs B because ...";
- "next: integrate slice N";
- "blocked: missing credential or human decision";
- "ready: merged PRs, baseline commit, proof path".

This is not a chat loop. It is live, structured status: phase, current task,
recent decisions, next step, blockers, gates, reviews, and proof links. The
user can observe and intervene, but should not need to steer every step.

## Quality Bar

The target is state-of-the-art delivery quality, not "agent output that still
needs a human polishing pass." Routine review, audit, refactor, cleanup,
documentation, and verification work are part of the goal.

If polish remains, the controller must keep working, create follow-up tasks, or
stop as `not_ready` / `blocked_on_human` with exact evidence. It must not mark
the goal `ready` while ordinary engineering cleanup is still expected.

## Safety Boundary

Automatic delivery requires explicit policy at goal creation or resume time.
The controller must not silently push, create PRs, merge, force-push, delete
branches, or modify protected baselines without a policy that allows the action.

Recommended policy shape:

```bash
omk goal run "<goal>" \
  --until-ready \
  --delivery auto-pr \
  --merge-policy gated \
  --max-agents 6
```

Policy meanings:

- `local`: local proof only; no network or GitHub mutation.
- `draft-pr`: push branches and create draft PRs, but do not merge.
- `auto-pr`: push branches and create/update PRs when gates pass.
- `gated`: merge only after proof, CI, and required reviews are green.
- `manual`: stop before merge and record the exact human action needed.

## Required End-to-End Flow

1. Intake the natural-language goal and constraints.
2. Inspect the repository and current protected baseline.
3. Classify the goal and define the oracle.
4. Generate goal brief, technical plan, test spec, and delivery plan.
5. Decompose the work into PR-sized slices.
6. Assign each slice an owner role, read/write scope, branch, worktree, gates,
   review wall, dependencies, and integration strategy.
7. Materialize task-scoped branches and worktrees.
8. Dispatch agents through the existing scheduler/Wire runtime.
9. Capture changed files, diffs, artifacts, and gate evidence per slice.
10. Commit accepted slice changes with task/proof metadata.
11. Push slice branches and create or update PRs when policy allows.
12. Run review loops on each slice PR.
13. Convert review findings into fix tasks and repeat until blockers clear.
14. Run cleanup/refactor passes when review or proof evidence shows rough edges.
15. Create or update an integrator branch/PR that combines accepted slices.
16. Detect merge conflicts and either resolve them or block with evidence.
17. Run the full verification wall against the integrated result.
18. Merge accepted PRs into `main` / `master` when policy and gates allow.
19. Record final proof with baseline commit, merged PRs, reviews, gates, known
    gaps, and any human decisions.

## Readiness Rule

In end-to-end delivery mode, `ready` means:

- every required task slice is complete or explicitly accepted as out of scope;
- every produced slice has branch, worktree, commit, PR, review, and gate
  evidence;
- blocking review findings are fixed or explicitly accepted by policy;
- the integrator branch/PR passed the full verification wall;
- all required PRs are merged into the protected baseline, or the policy is
  explicitly `manual` and the final proof says exactly what remains unmerged;
- `proof.json` records the final baseline branch and commit.

Intermediate states such as "local gates passed", "PR draft rendered", or
"agent output exists" are useful evidence, but they are not complete readiness.

## Data Model Additions

Extend goal and task graph artifacts with delivery evidence:

- `delivery_policy`: local, draft-pr, auto-pr, or manual.
- `merge_policy`: gated, manual, or disabled.
- `slice_id`: stable id for each PR-sized work unit.
- `owner_role`: agent or human role responsible for the slice.
- `worktree_path`: local path for the task-scoped worktree.
- `branch_name`: task branch name.
- `commit_ids`: commits produced for the slice.
- `pr_number`, `pr_url`, `pr_state`, `pr_review_state`.
- `review_attempts`: review/fix loop history.
- `ci_runs`: CI identifiers and conclusions.
- `integration_branch`, `integration_pr_url`, `integration_commit`.
- `merge_result`: merged, rejected, blocked, skipped, or manual.

The proof bundle must summarize this metadata and link to durable artifacts.

## Review Wall

Each slice and the final integrator result must pass a review wall appropriate
to the goal:

- architect review for boundary and design risks;
- code review for correctness and maintainability;
- test-engineer review for coverage and flake risk;
- security review for trust boundaries, secrets, and dependency risks;
- performance review when latency, memory, or throughput are relevant;
- anti-slop cleanup/refactor review before final integration.

Review findings become normal task graph nodes with owners, write scopes, gates,
and proof evidence. The controller repeats the review/fix loop until the review
wall is clean, policy accepts a known gap, or the goal blocks.

## GitHub Delivery

PR bodies generated by `omk goal` must include:

- goal id and slice id;
- owner role and write scope;
- branch and worktree evidence;
- task acceptance criteria;
- verification wall output summary;
- review wall status;
- known gaps and rejected alternatives;
- links to proof, decisions, artifacts, and CI runs.

The controller should prefer draft PRs until a slice passes its verification and
review wall. It may mark PRs ready for review or merge only when policy allows.

## Integrator Flow

The integrator is a first-class task, not a summary step.

Responsibilities:

- combine accepted task slices in dependency order;
- preserve docs, changelog, migrations, and release notes;
- detect and resolve safe merge conflicts;
- rerun the verification wall on the combined tree;
- create/update the integration PR;
- record final merge evidence or blocker evidence.

If automatic conflict resolution is unsafe, the goal must stop as
`blocked_on_human` or `not_ready` with the conflict files, branch names, and
recommended recovery steps.

## Acceptance Tests

End-to-end delivery is not considered implemented until tests or fixtures prove:

- a fresh user can install, run one goal command, and observe progress through
  terminal/TUI output;
- a goal decomposes into at least two independent delivery slices;
- the controller materializes separate worktrees and task branches;
- two agents can work on non-overlapping slices;
- slice changes are committed with task/proof metadata;
- PR output includes task id, owner, write scope, gates, reviews, and gaps;
- review findings create follow-up fix tasks and repeat the loop;
- an integrator task combines accepted slices and reruns gates;
- conflict detection blocks unsafe integration with clear evidence;
- final proof records merged PRs and the protected baseline commit;
- local/dry-run policy never performs network or GitHub mutation.

## Out of Scope

- Silent force-pushes.
- Merging with failing required CI.
- Deleting user branches without explicit policy.
- Claiming product-market or UX success from synthetic tests alone.
- Bypassing repository branch protection or human-required reviews.
