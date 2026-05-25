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

## Changelog

<!--
Every user-visible change requires a CHANGELOG.md entry in the
[Unreleased] section before merge. If this PR is internal-only
(refactor, test, CI hygiene with zero user impact), say so and
skip the entry.
-->

- [ ] Added an entry to `CHANGELOG.md` `## [Unreleased]` section
      *(or)*
- [ ] N/A — internal-only change (refactor / test / CI). Explain:
      <reason>

Version impact (pre-1.0 semver):

- [ ] No version bump needed
- [ ] Patch bump on next release (bugfix, no surface change)
- [ ] Minor bump on next release (new feature, possibly breaking)
- [ ] Major bump on next release (1.0 commit / large breaking)

## Documentation

<!--
If this PR adds/changes user-facing surface (CLI commands,
flags, configuration keys, public Rust API, file formats,
events/metrics), associated docs must be updated in the same PR.
-->

- [ ] `README.md` updated (or N/A — no surface change)
- [ ] `docs/` files updated (or N/A — no doc surface affected)
- [ ] Module-level `README.md` / `AGENTS.md` updated for any
      touched `src/X/` (per AGENTS.md §Agent Module Architecture)

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
