## Task

<!-- Required for multi-agent work. Example: goal-agent-implement, docs-refresh, omk-123 if an external tracker is used. -->

- Task:
- Owner:
- Branch:

## Summary

<!-- What changed, and why? -->

## Scope

<!-- List owned files/modules. Call out overlaps with other active work. -->

- Write scope:
- Dependencies / blockers:

## Verification

<!-- Paste exact commands and result. Mark intentionally skipped gates. -->

- [ ] `cargo fmt --check`
- [ ] `git diff --check`
- [ ] `cargo check --all-targets --all-features`
- [ ] `cargo clippy --all-targets --all-features -- -D warnings`
- [ ] `cargo test --all-features`
- [ ] `cargo doc --no-deps`
- [ ] `cargo deny check advisories licenses sources --all-features`
- [ ] `cargo run --bin validate-contracts`

## Risk

- Scope risk: <!-- narrow / moderate / broad -->
- Rollback plan:
- Known gaps:

## Checklist

- [ ] This PR is not targeting direct work on `master` / `main` outside the PR flow.
- [ ] The task/scope and owner are declared above.
- [ ] Docs/changelog/version were updated when behavior or release metadata changed.
- [ ] New behavior has tests or an explicit `Not-tested:` rationale.
