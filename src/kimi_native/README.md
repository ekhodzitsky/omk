# Kimi Native Area Map

`src/kimi_native/` owns project/user Kimi assets and the OMK manifest that tracks what was installed.

Official source of truth:

- Kimi docs root: https://www.kimi.com/code/docs
- Kimi skills: https://www.kimi.com/code/docs/en/kimi-code-cli/customization/skills.html
- Kimi subagents: https://www.kimi.com/code/docs/en/kimi-code-cli/customization/sub-agents.html

## Files

| File | Owns |
| --- | --- |
| `agent_spec.rs` | Generated Kimi agent specs and default role agents. |
| `diagnostics.rs` | Kimi-native doctor checks. |
| `hook_spec.rs` | Default lifecycle hook definitions. |
| `installer.rs` | Asset installation helpers. |
| `manifest.rs` | Managed asset manifest, checksums, rollback helpers. |
| `role_packs.rs` | Curated role pack metadata. |
| `sync.rs` | Project/user asset reconciliation. |

## Edit Rules

- Treat `.kimi/` as user-visible project configuration. Back up before overwriting.
- Manifest entries must be precise enough for doctor/rollback to explain ownership.
- Prefer project-level `.kimi/skills/` for repo navigation behavior that should help Kimi work on this repository.
- Keep generated assets compatible with official Kimi CLI docs, not only with OMK assumptions.
- `omk kimi rollback` is manifest-driven: if the manifest is missing, rollback is a clean non-fatal no-op with an informational message.

## Tests

Start here:

```bash
cargo test --test kimi_native_test
cargo test --test role_pack_test
```

For sync/doctor behavior, also run the relevant `omk kimi ...` command manually when a local Kimi installation is present.
