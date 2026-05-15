# skills TODO

## Current
- [ ] Add unit tests for `find_skill` (discovery.rs).
- [ ] Add unit tests for `inject_skill` and `match_trigger` (injector.rs).
- [ ] Reconcile `cli/skill.rs` with `src/skills/` so the CLI uses the module's discovery/parser surface instead of raw filesystem operations.

## Next
- [ ] Add integration test for `discover_skills` with a temporary directory fixture.
- [ ] Add schema validation or golden test for `Skill` serialization roundtrip.
