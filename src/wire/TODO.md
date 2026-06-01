# wire — TODO

## Current
- [x] Migrate all generic wire logic to the external `kimi-wire` crate.
- [x] Reduce `src/wire/` to a thin re-export + compatibility alias layer.
- [x] Delete `src/wire/dispatch.rs` (moved to `kimi-wire::dispatch`).

## Next
- [ ] Migrate remaining call sites from `redact_wire_secrets` to `kimi_wire::redact_secrets`, then remove the wrapper.
- [ ] Migrate `ApprovalResponseType` alias call sites to `ApprovalResponseKind`, then remove the alias.
- [ ] Property-based tests for WireMessage serde roundtrip (in `kimi-wire`).
- [ ] Golden tests for new protocol version messages (in `kimi-wire`).
