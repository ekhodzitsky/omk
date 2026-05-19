# kimi_native — Agent Guide

## Editing Rules

1. **Install is append-only by default.** `install_project_assets` and
   `install_user_assets` create backups before overwriting existing files.
   Backups are named `<file>.omk-backup.<timestamp>`.
2. **Manifest is the source of truth.** `KimiNativeManifest` tracks ownership
   of installed files. Do not mutate `.kimi/` contents without updating the
   manifest and emitting an `asset_changed` event.
3. **No destructive ops without confirmation.** `--force` is required to delete
   existing user modifications. Default behavior refuses destructive changes.
4. **Assets are versioned.** The manifest records `schema_version`. Migration
   logic lives in `migrate.rs` and must handle one-version-at-a-time upgrades.
5. **Test through temp dirs.** Integration tests use `tempfile::TempDir` as the
   project root. All assertions verify both filesystem state and manifest state.
