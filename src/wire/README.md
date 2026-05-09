# Wire Area Map

`src/wire/` contains the Kimi Wire Protocol contract used to talk to `kimi --wire` through structured JSON-RPC messages.
OMK currently pins Wire protocol `1.9`, and the client falls back to legacy/no-handshake mode when upstream returns `method-not-found` for `initialize`.

Official source of truth: https://www.kimi.com/code/docs/en/kimi-code-cli/customization/wire-protocol.html

## Files

| File | Owns |
| --- | --- |
| `mod.rs` | Wire module exports. |
| `protocol.rs` | JSON-RPC request/response/event types, protocol version helpers, parsing. |
| `client.rs` | Process/client adapter for `kimi --wire`. |

## Edit Rules

- Check the official Kimi Wire docs before changing protocol fields, method names, or fallback behavior.
- Record Kimi CLI version and negotiated Wire protocol version when the runtime observes them.
- Prefer strongly typed protocol structs over ad hoc JSON strings.
- Keep prompt-scraping as a fallback path, not the primary contract.
- Preserve the legacy/no-handshake fallback for older upstream behavior.
- Add fixtures when changing parse behavior.

## Tests

Start here:

```bash
cargo test --test wire_protocol_test
bash -n scripts/kimi-wire-smoke.sh
```

If a local authenticated `kimi` binary is available, also run:

```bash
scripts/kimi-wire-smoke.sh
```

The smoke script is intentionally outside `cargo test` because it depends on the user's Kimi installation and auth state.
