---
schema_version: 1
module: marketplace
level: root
purpose: Fetch and merge external skill marketplace registries.
status: stable
surface:
  - name: load_all_skills
    kind: fn
    visibility: pub
    contract: >
      Given a list of registry URLs or file paths, fetches each registry and
      returns a flattened list of (registry_name, skill) tuples.
      Non-fatal errors are logged and skipped.
    proof:
      kind: integration-test
      target: tests/marketplace_test.rs
      command: cargo test --test marketplace_test
  - name: MarketplaceRegistry
    kind: struct
    visibility: pub
    contract: >
      In-memory representation of a marketplace registry with name, URL, and skills list.
    proof:
      kind: integration-test
      target: tests/marketplace_test.rs
      command: cargo test --test marketplace_test
  - name: MarketplaceRegistry::fetch
    kind: fn
    visibility: pub
    contract: >
      HTTP GET a registry JSON with a 30-second timeout,
      returning a parsed MarketplaceRegistry or an error.
    proof:
      kind: integration-test
      target: tests/marketplace_test.rs
      command: cargo test --test marketplace_test
  - name: MarketplaceRegistry::fetch_file
    kind: fn
    visibility: pub
    contract: >
      Read and parse a local registry JSON file asynchronously.
    proof:
      kind: integration-test
      target: tests/marketplace_test.rs
      command: cargo test --test marketplace_test
  - name: RegistrySkill
    kind: struct
    visibility: pub
    contract: >
      Individual skill entry with name, description, author, URL, and tags.
    proof:
      kind: schema
      target: src/marketplace/registry.rs
      command: cargo check
dependencies:
  internal: []
  external:
    - name: anyhow
      scope: error handling
      reason: Structured error propagation for HTTP and file I/O.
    - name: serde
      scope: serialization
      reason: Deserialize registry and skill JSON.
    - name: reqwest
      scope: HTTP client
      reason: Fetch remote marketplace registries over HTTP/HTTPS.
    - name: tokio
      scope: async file I/O
      reason: Non-blocking filesystem reads for local registries.
    - name: tracing
      scope: observability
      reason: Warn-level logging for registry fetch failures.
    - name: std
      scope: path and time
      reason: Path handling and HTTP timeout configuration.
consumers:
  - path: src/cli/marketplace.rs
    uses: ["load_all_skills", "MarketplaceRegistry::fetch", "MarketplaceRegistry::fetch_file"]
  - path: src/cli/doctor.rs
    uses: ["MarketplaceRegistry::fetch", "MarketplaceRegistry::fetch_file"]
invariants:
  - id: timeout-bounded
    rule: HTTP registry fetches use a 30-second timeout to prevent indefinite hangs.
    proof:
      kind: static-check
      target: src/marketplace/registry.rs
      command: cargo clippy --all-targets --all-features -- -D warnings
  - id: best-effort-loading
    rule: load_all_skills skips registries that fail instead of failing the entire operation.
    proof:
      kind: static-check
      target: src/marketplace/registry.rs
      command: cargo clippy --all-targets --all-features -- -D warnings
  - id: no-panic
    rule: Public functions do not panic; errors are propagated via Result.
    proof:
      kind: static-check
      target: src/marketplace
      command: cargo clippy --all-targets --all-features -- -D warnings
verification:
  pre_change:
    - cargo test --lib marketplace
  full:
    - cargo test
    - cargo clippy --all-targets --all-features -- -D warnings
---

# marketplace

## Architecture

The `marketplace` module is a thin I/O layer for external skill registries.

- `registry.rs` owns the data model (`MarketplaceRegistry`, `RegistrySkill`) and two fetch strategies: remote HTTP and local file.
- `mod.rs` is a storefront: it declares the submodule and re-exports the public surface.

Data flow: CLI provides a list of registry URLs/paths → `load_all_skills` iterates → dispatches to `fetch` (HTTP) or `fetch_file` (local) → returns flattened skills. Failures are logged and skipped.

## Files

| File | Responsibility |
|------|----------------|
| `mod.rs` | Storefront: declares submodule, re-exports surface. |
| `registry.rs` | Data model, HTTP fetch, file fetch, and skill flattening. |
