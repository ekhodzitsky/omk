# kimi_native TODO

## Contract gaps

- [ ] `install_user_assets` is `#[allow(dead_code)]` and has no CLI command surface. Decide whether to expose via `omk kimi install --user` or remove.
- [ ] `AssetManifest::verify_checksum` is `#[allow(dead_code)]`. Determine if it should be used by doctor or removed.
- [ ] `AssetManifest::schema_version` is `#[allow(dead_code)]`. Either wire into a CLI diagnostic or drop.
- [ ] `manifest::path::absolute_root` and `to_project_relative` are `pub(crate)` but only used inside manifest. No action needed unless external callers appear.

## Coverage gaps

- [ ] No dedicated unit tests for `agent_spec::write_agent_to_dir` (covered only by integration tests).
- [ ] No unit tests for `hook_spec::default_project_hooks` script content assertions.
- [ ] `diagnostics::cli::check_kimi_cli` depends on external `kimi` binary; add a mock-path test or document as manual-only.
- [ ] No benchmark for manifest drift detection on large file sets.

## Potential refactors

- [ ] Consider deduplicating agent spec construction logic shared between `installer.rs` and `sync.rs` into a shared helper.
- [ ] `rollback.rs` and `manifest::ops.rs` both define `RollbackReport` with different fields. Consider renaming one to avoid confusion.
- [ ] `diagnostics` submodules use `crate::kimi_native::...` absolute paths; this is fine but some use `super::super::` in tests (legacy). Clean up if touched.
