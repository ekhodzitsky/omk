# TODO — cli

## Current
- [ ] Monitor file sizes: `app/update.rs` (352), `team/run.rs` (355), `goal/mod.rs` (382), `goal/commands/mod.rs` (355) — split if any exceeds 400 lines.
- [ ] Add unit tests for CLI argument parsing (currently only integration tests in `tests/cli_smoke.rs` and inline tests in `cleanup.rs`, `goal/validate.rs`, `team/proof.rs`).

## Next
- [ ] Evaluate whether `Omk` / `Commands` should become `pub` for external programmatic use (currently `pub(super)`).
- [ ] Extract shared TUI/web feature-flag logic if `vis` integration grows.
