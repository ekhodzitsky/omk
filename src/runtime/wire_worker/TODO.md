# runtime::wire_worker TODO

## Current
- [x] Document module contract.
- [ ] Add unit-test proof for `poll_interval` env resolution edge cases (zero/invalid values).
- [ ] Add unit-test proof for `resolve_active_turn_timeout` env resolution.
- [ ] Move `task_timeout_secs` logic into a typed helper or method on `WorkerTask` to make the default visible in the contract.

## Next
- [ ] Add golden/schema proof for WorkerResult and heartbeat JSON shapes emitted by this module.
- [ ] Evaluate whether `ProcessWireClient` should be injected as a port so the adapter can be unit-tested without a real child process.
