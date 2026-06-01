# wire — Agent Guide

## Editing Rules

1. **This module is a thin re-export layer.** All wire protocol logic lives in
   the external `kimi-wire` crate. Do not add parsing, dispatch, or trait
   implementations to `src/wire/`.
2. **Compatibility aliases only.** `ProcessWireClient`, `ApprovalResponseType`,
   `redact_wire_secrets`, and `KIMI_WIRE_PROTOCOL_VERSION` are preserved so
   existing OMK call sites keep compiling. Migrate call sites to the upstream
   names and remove the alias when practical.
3. **Upstream changes require a `kimi-wire` PR.** If you need to change
   `WireClient`, `process_messages`, protocol types, or redaction logic, edit
   the `kimi-wire` crate (path: `../kimi-wire`) and bump its version before
   updating OMK's dependency.
4. **Protocol facts must not go stale.** When changing re-exports or aliases,
   update `README.md`, `docs/`, and consumer tests in the same PR.
