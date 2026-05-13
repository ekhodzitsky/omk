## Bead

<!-- Required for multi-agent work. Example: Closes bd:omk-123 -->

- Bead:
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

- [ ] `cargo fmt -- --check`
- [ ] `git diff --check`
- [ ] `cargo check --all-targets`
- [ ] `cargo clippy --all-targets --all-features -- -D warnings`
- [ ] `cargo test --all-features`
- [ ] `cargo doc --no-deps`
- [ ] `cargo deny --all-features check advisories licenses`

## Risk

- Scope risk: <!-- narrow / moderate / broad -->
- Rollback plan:
- Known gaps:

## Checklist

- [ ] This PR is not targeting direct work on `master` / `main` outside the PR flow.
- [ ] The bead has been claimed or this PR explains why no bead applies.
- [ ] Docs/changelog/version were updated when behavior or release metadata changed.
- [ ] New behavior has tests or an explicit `Not-tested:` rationale.
