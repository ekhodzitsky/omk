---
schema_version: 1
module: kimi_native
level: root
purpose: Manage Kimi CLI project assets (agents, hooks, skills) and track ownership via a durable manifest.
status: stable
surface:
  - name: install_project_assets
    kind: fn
    visibility: pub
    contract: >
      Installs default role agents, hook scripts, and a hooks.toml.example into
      `.kimi/` under the given project directory. Creates backups before overwriting
      existing files. Returns an InstallReport summarizing changes.
    proof:
      kind: integration-test
      target: tests::kimi_native_test
      command: cargo test --test kimi_native_test
  - name: install_user_assets
    kind: fn
    visibility: pub
    contract: >
      Installs default role agents into the user's Kimi config directory
      (`~/.config/kimi/`). Currently unused by the CLI but kept as a pub helper.
    proof:
      kind: manual
      target: src/kimi_native/installer.rs
      command: ""
  - name: sync_project_assets
    kind: fn
    visibility: pub
    contract: >
      Reconciles OMK assets with `.kimi/` by comparing checksums. Creates, updates,
      or skips files; backs up user-modified files before overwriting. Persists an
      AssetManifest tracking ownership.
    proof:
      kind: integration-test
      target: tests::kimi_native_test
      command: cargo test --test kimi_native_test
  - name: sync_user_assets
    kind: fn
    visibility: pub
    contract: >
      User-level variant of sync_project_assets targeting `~/.config/kimi/`.
      Skips identical files and respects force/dry-run flags.
    proof:
      kind: integration-test
      target: tests::kimi_native_test
      command: cargo test --test kimi_native_test
  - name: rollback
    kind: fn
    visibility: pub
    contract: >
      Rolls back OMK-managed assets using the AssetManifest. Restores from backup
      when available; removes OMK-created files matching manifest checksums;
      skips user-modified files. Returns a clean no-op if no manifest exists.
    proof:
      kind: integration-test
      target: tests::kimi_native_test
      command: cargo test --test kimi_native_test
  - name: diagnose_project
    kind: fn
    visibility: pub
    contract: >
      Runs Kimi-native doctor checks against `.kimi/`: agents, hooks, skills,
      manifest drift, AGENTS.md presence, and Kimi CLI availability. Returns
      a vector of DiagResult with severity and fix hints.
    proof:
      kind: integration-test
      target: tests::kimi_native_test
      command: cargo test --test kimi_native_test
  - name: AgentSpec
    kind: struct
    visibility: pub
    contract: >
      Top-level agent spec struct compatible with Kimi CLI's AGENTS.md format.
      Serializes to YAML via to_yaml().
    proof:
      kind: unit-test
      target: kimi_native::manifest::tests
      command: cargo test --lib kimi_native
  - name: RoleAgent
    kind: struct
    visibility: pub
    contract: >
      Internal OMK role definition used to generate Kimi agent files.
      Contains id, name, system prompt, and tool list.
    proof:
      kind: integration-test
      target: tests::kimi_native_test
      command: cargo test --test kimi_native_test
  - name: default_role_agents
    kind: fn
    visibility: pub
    contract: >
      Returns the six built-in role agents (architect, executor, verifier,
      reviewer, security, explore) with their system prompts and tool bindings.
    proof:
      kind: integration-test
      target: tests::kimi_native_test
      command: cargo test --test kimi_native_test
  - name: write_agent_to_dir
    kind: fn
    visibility: pub
    contract: >
      Writes an agent.yaml and system.md for a single RoleAgent into the
      specified directory using atomic writes.
    proof:
      kind: integration-test
      target: tests::kimi_native_test
      command: cargo test --test kimi_native_test
  - name: HookEvent
    kind: enum
    visibility: pub
    contract: >
      Kimi CLI hook event types (PreToolUse, PostToolUse, Stop, SessionStart, etc).
      Serialized with PascalCase.
    proof:
      kind: integration-test
      target: tests::kimi_native_test
      command: cargo test --test kimi_native_test
  - name: HookConfig
    kind: struct
    visibility: pub
    contract: >
      A single hook definition binding an event to a shell command with optional
      matcher regex and timeout.
    proof:
      kind: integration-test
      target: tests::kimi_native_test
      command: cargo test --test kimi_native_test
  - name: ProjectHookDefs
    kind: struct
    visibility: pub
    contract: >
      Collection of all hooks and their script contents that OMK recommends for
      a project. Used by installer and sync.
    proof:
      kind: integration-test
      target: tests::kimi_native_test
      command: cargo test --test kimi_native_test
  - name: default_project_hooks
    kind: fn
    visibility: pub
    contract: >
      Returns the three default hooks (safety-check, completion-check, notify)
      with their shell script contents.
    proof:
      kind: integration-test
      target: tests::kimi_native_test
      command: cargo test --test kimi_native_test
  - name: AssetManifest
    kind: struct
    visibility: pub
    contract: >
      Durable record of all files and directories OMK manages under `.kimi/`,
      including checksums and backup index. Saved as `.kimi/omk-manifest.json`.
    proof:
      kind: unit-test
      target: kimi_native::manifest::tests
      command: cargo test --lib kimi_native
  - name: EntryKind
    kind: enum
    visibility: pub
    contract: >
      Classification of manifest entries (agent_spec, agent_prompt, hook_script,
      hook_config, skill, config, other).
    proof:
      kind: unit-test
      target: kimi_native::manifest::tests
      command: cargo test --lib kimi_native
  - name: compute_checksum
    kind: fn
    visibility: pub
    contract: >
      Computes a 64-bit FNV-1a checksum formatted as a 16-char hex string.
      Used for drift detection and rollback safety.
    proof:
      kind: unit-test
      target: kimi_native::manifest::tests::test_compute_checksum
      command: cargo test --lib kimi_native::manifest::tests::test_compute_checksum
  - name: is_identical
    kind: fn
    visibility: pub
    contract: >
      Compares a file on disk with a string by byte equality or checksum match.
    proof:
      kind: unit-test
      target: kimi_native::manifest::tests
      command: cargo test --lib kimi_native
  - name: maybe_backup
    kind: fn
    visibility: pub
    contract: >
      Creates a timestamped `.omk-backup-{ts}` copy of an existing file if its
      content differs from the new content. Returns None if no backup was needed.
    proof:
      kind: integration-test
      target: tests::kimi_native_test
      command: cargo test --test kimi_native_test
  - name: RolePack
    kind: struct
    visibility: pub
    contract: >
      Curated role pack metadata (id, name, system prompt, tools, default skills,
      suggested worker count) used by team run to resolve role aliases.
    proof:
      kind: integration-test
      target: tests::role_pack_test
      command: cargo test --test role_pack_test
  - name: DiagResult
    kind: struct
    visibility: pub
    contract: >
      Single diagnostic result with severity, message, and optional fix hint.
    proof:
      kind: integration-test
      target: tests::kimi_native_test
      command: cargo test --test kimi_native_test
  - name: Severity
    kind: enum
    visibility: pub
    contract: >
      Diagnostic severity level: Ok, Warning, Error.
    proof:
      kind: integration-test
      target: tests::kimi_native_test
      command: cargo test --test kimi_native_test
  - name: InstallReport
    kind: struct
    visibility: pub
    contract: >
      Summary of an install operation: agents installed, hooks installed,
      skills linked, errors, backups created, and dry-run plan.
    proof:
      kind: integration-test
      target: tests::kimi_native_test
      command: cargo test --test kimi_native_test
  - name: SyncReport
    kind: struct
    visibility: pub
    contract: >
      Summary of a sync operation with created/updated/unchanged/would-create/would-update
      lists, backup tracking, and project vs user scope.
    proof:
      kind: integration-test
      target: tests::kimi_native_test
      command: cargo test --test kimi_native_test
  - name: SyncScope
    kind: enum
    visibility: pub
    contract: >
      Distinguishes project-level vs user-level sync operations.
    proof:
      kind: integration-test
      target: tests::kimi_native_test
      command: cargo test --test kimi_native_test
  - name: RollbackReport
    kind: struct
    visibility: pub
    contract: >
      Summary of a rollback operation: restored files, removed files, skipped files,
      errors, and whether the manifest was missing (clean no-op).
    proof:
      kind: integration-test
      target: tests::kimi_native_test
      command: cargo test --test kimi_native_test
dependencies:
  internal:
    - module: runtime::atomic
      scope: file I/O
      reason: Atomic writes for agent specs, prompts, hooks, and manifest JSON.
    - module: runtime::config
      scope: path resolution
      reason: Resolve OMK data directory for skills symlink source.
  external:
    - name: anyhow
      scope: error handling
      reason: Structured error propagation across async install/sync/rollback/doctor.
    - name: serde / serde_json / serde_yaml
      scope: serialization
      reason: Agent specs (YAML), manifest (JSON), diagnostics (JSON), hook configs.
    - name: tokio
      scope: async file I/O
      reason: Non-blocking filesystem operations for all asset management.
    - name: tracing
      scope: observability
      reason: Structured logging for install, sync, rollback, and diagnostics.
    - name: chrono
      scope: timestamps
      reason: Manifest created_at and backup entry timestamps.
    - name: dirs
      scope: path resolution
      reason: User config directory for user-level asset installation.
    - name: which
      scope: diagnostics
      reason: Detect Kimi CLI presence in PATH during doctor checks.
    - name: toml
      scope: diagnostics
      reason: Parse hooks.toml.example and config.toml for dangling references.
consumers:
  - path: src/cli/kimi_native_cmd/install.rs
    uses: ["installer::install_project_assets"]
  - path: src/cli/kimi_native_cmd/sync.rs
    uses: ["sync::sync_project_assets", "sync::sync_user_assets"]
  - path: src/cli/kimi_native_cmd/rollback.rs
    uses: ["rollback::rollback"]
  - path: src/cli/kimi_native_cmd/doctor.rs
    uses: ["diagnostics::diagnose_project", "diagnostics::Severity"]
  - path: src/cli/kimi_native_cmd/agents.rs
    uses: ["agent_spec::default_role_agents"]
  - path: src/cli/kimi_native_cmd/hooks.rs
    uses: ["hook_spec::default_project_hooks"]
  - path: src/cli/team/args.rs
    uses: ["role_packs::RolePack"]
  - path: src/cli/team/run.rs
    uses: ["role_packs::RolePack::find"]
invariants:
  - id: manifest-path-safety
    rule: AssetManifest load rejects parent traversal, absolute paths, and paths escaping the project root.
    proof:
      kind: unit-test
      target: kimi_native::manifest::tests
      command: cargo test --lib kimi_native::manifest::tests
  - id: rollback-no-manifest-is-noop
    rule: rollback returns a clean no-op report with manifest_missing=true when no manifest exists.
    proof:
      kind: unit-test
      target: kimi_native::rollback::tests
      command: cargo test --lib kimi_native::rollback
  - id: checksum-drift-detection
    rule: Manifest drifted_files detects both missing files and checksum mismatches.
    proof:
      kind: unit-test
      target: kimi_native::manifest::tests
      command: cargo test --lib kimi_native::manifest::tests
  - id: dry-run-no-side-effects
    rule: sync and rollback dry_run modes report planned changes without modifying disk.
    proof:
      kind: integration-test
      target: tests::kimi_native_test
      command: cargo test --test kimi_native_test
  - id: role-packs-include-guards
    rule: Every built-in role pack prompt contains Instruction Hierarchy, AGENTS.md, Anti-Slop, and Review Discipline.
    proof:
      kind: integration-test
      target: tests::role_pack_test
      command: cargo test --test role_pack_test
  - id: backup-before-overwrite
    rule: sync and install create timestamped backups before overwriting existing files that differ.
    proof:
      kind: integration-test
      target: tests::kimi_native_test
      command: cargo test --test kimi_native_test
verification:
  pre_change:
    - cargo test --lib kimi_native
  full:
    - cargo test
    - cargo clippy --all-targets --all-features -- -D warnings
---

# kimi_native

## Architecture

The `kimi_native` module is OMK's integration layer with the Kimi CLI ecosystem.
It generates, installs, synchronizes, and diagnoses project-level Kimi assets
(agents, hooks, skills) under `.kimi/`, and maintains a durable `omk-manifest.json`
to track ownership for safe rollback.

```
┌─────────────────────────────────────────────────────────────┐
│                         CLI layer                           │
│   install │ sync │ rollback │ doctor │ agents │ hooks      │
└─────────────┬─────────────┬─────────────┬───────────────────┘
              │             │             │
    ┌─────────▼──────┐ ┌───▼────┐ ┌──────▼──────┐
    │   installer    │ │  sync  │ │  rollback   │
    └─────────┬──────┘ └───┬────┘ └──────┬──────┘
              │            │             │
    ┌─────────▼────────────▼─────────────▼──────┐
    │              manifest / atomic            │
    │  AssetManifest · checksum · backup · I/O  │
    └───────────────────────────────────────────┘
              │
    ┌─────────▼─────────────────────────────────┐
    │  agent_spec · hook_spec · role_packs      │
    │  Default assets and metadata definitions   │
    └───────────────────────────────────────────┘
              │
    ┌─────────▼─────────────────────────────────┐
    │           diagnostics                      │
    │  agents · hooks · skills · manifest · cli  │
    └───────────────────────────────────────────┘
```

## Files

| File | Responsibility |
|------|----------------|
| `mod.rs` | Storefront: 8 lines, declares all submodules. |
| `agent_spec.rs` | Kimi agent YAML spec types and default role agents. |
| `hook_spec.rs` | Hook event enum, config struct, and default project hooks. |
| `installer.rs` | `install_project_assets` and `install_user_assets` with backup logic. |
| `sync.rs` | `sync_project_assets` and `sync_user_assets` with checksum-based skip. |
| `rollback.rs` | Manifest-driven rollback with backup restore and safe removal. |
| `role_packs.rs` | Curated role pack metadata for team run alias resolution. |
| `diagnostics/mod.rs` | Top-level `diagnose_project` orchestrating all checks. |
| `diagnostics/types.rs` | `DiagResult` and `Severity` types. |
| `diagnostics/agents.rs` | Agent spec validation and presence checks. |
| `diagnostics/hooks.rs` | Hook script presence, executability, and config validation. |
| `diagnostics/skills.rs` | Skills directory validation (SKILL.md per entry). |
| `diagnostics/assets.rs` | AGENTS.md presence and manifest drift checks. |
| `diagnostics/cli.rs` | Kimi CLI version and availability check. |
| `manifest/mod.rs` | Storefront: re-exports manifest types and checksum helpers. |
| `manifest/types.rs` | `AssetManifest`, `ManifestEntry`, `BackupEntry`, `EntryKind`. |
| `manifest/ops.rs` | `AssetManifest` methods: add_file, save, load, rollback, drifted_files. |
| `manifest/checksum.rs` | FNV-1a checksums and `is_identical` / `maybe_backup` helpers. |
| `manifest/path.rs` | Project-relative path normalization and traversal validation. |
| `manifest/tests.rs` | Manifest unit tests: checksum, drift, schema version, path safety. |

## Edit Rules

- Treat `.kimi/` as user-visible project configuration. Back up before overwriting.
- Manifest entries must be precise enough for doctor/rollback to explain ownership.
- Prefer project-level `.kimi/skills/` for repo navigation behavior that should help Kimi work on this repository.
- Keep generated assets compatible with official Kimi CLI docs, not only with OMK assumptions.
- `omk kimi rollback` is manifest-driven: if the manifest is missing, rollback is a clean non-fatal no-op with an informational message.
