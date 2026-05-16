# TODO — runtime

## Active
- [ ] `runtime/ask/` needs contract tests for each provider command path.
- [ ] `runtime/autopilot/` needs unit tests for phase transition logic.
- [ ] `runtime/gates/` needs schema validation for `.gates.toml` files.
- [ ] `runtime/proof/` needs golden tests for proof JSON output shape.
- [ ] `runtime/ralph/` needs timeout and retry configurability exposed via CLI.
- [ ] `runtime/scheduler/` needs in-memory simulation tests for `OwnershipMap` races.

## Later
- Consolidate shared retry/backoff logic between `ralph`, `autopilot`, and `scheduler`.
