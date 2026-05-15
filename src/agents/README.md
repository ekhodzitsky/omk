---
schema_version: 1
module: agents
level: root
purpose: Parse and load AGENTS.md files, inject agent context into prompts.
status: stable
surface:
  - name: load_project_agents
    kind: fn
    visibility: pub
    contract: >
      Walks from start_dir up to the filesystem root and returns the first
      AGENTS.md manifest found, or None if no manifest exists.
    proof:
      kind: unit-test
      target: agents::parser::tests
      command: cargo test --lib agents::parser::tests
  - name: inject_agents_context
    kind: fn
    visibility: pub
    contract: >
      Builds a formatted Markdown context string from an AgentsManifest,
      task description, and role for prompt enrichment.
    proof:
      kind: integration-test
      target: runtime consumers (ralph, autopilot, ultrawork)
      command: cargo test
  - name: AgentsManifest
    kind: struct
    visibility: pub
    contract: >
      Parsed AGENTS.md manifest with optional name, description,
      agent roles, and free-form body.
    proof:
      kind: unit-test
      target: agents::parser::tests::test_parse_agents_md
      command: cargo test --lib agents::parser::tests::test_parse_agents_md
  - name: AgentRole
    kind: struct
    visibility: pub
    contract: >
      Individual agent role with name, description, and optional tier.
    proof:
      kind: unit-test
      target: agents::parser::tests::test_parse_agents_md
      command: cargo test --lib agents::parser::tests::test_parse_agents_md
  - name: parse_agents_md
    kind: fn
    visibility: pub
    contract: >
      Splits AGENTS.md content into YAML frontmatter and Markdown body,
      then deserializes the frontmatter into an AgentsManifest.
    proof:
      kind: unit-test
      target: agents::parser::tests::test_parse_agents_md
      command: cargo test --lib agents::parser::tests::test_parse_agents_md
  - name: default_agents_md
    kind: fn
    visibility: pub
    contract: >
      Returns a static default AGENTS.md template string used by omk setup.
    proof:
      kind: manual
      target: src/agents/runtime.rs
      command: ""
dependencies:
  internal: []
  external:
    - name: anyhow
      scope: error handling
      reason: Structured error propagation for parsing and I/O.
    - name: serde / serde_yaml
      scope: parsing
      reason: Deserialize YAML frontmatter in AGENTS.md.
    - name: tokio
      scope: async file I/O
      reason: Non-blocking filesystem reads for AGENTS.md.
    - name: std
      scope: path and string handling
      reason: Core filesystem and string operations.
consumers:
  - path: src/runtime/ultrawork.rs
    uses: ["load_project_agents", "inject_agents_context"]
  - path: src/runtime/autopilot/engine/mod.rs
    uses: ["load_project_agents", "inject_agents_context"]
  - path: src/runtime/ralph/engine.rs
    uses: ["load_project_agents", "inject_agents_context"]
  - path: src/cli/app/setup.rs
    uses: ["runtime::default_agents_md"]
  - path: src/cli/doctor.rs
    uses: ["parser::parse_agents_md"]
invariants:
  - id: frontmatter-optional
    rule: AGENTS.md without YAML frontmatter parses successfully with a default manifest.
    proof:
      kind: unit-test
      target: agents::parser::tests::test_parse_no_frontmatter
      command: cargo test --lib agents::parser::tests::test_parse_no_frontmatter
  - id: upward-search-terminates
    rule: load_project_agents walks parent directories and terminates at the filesystem root.
    proof:
      kind: static-check
      target: src/agents/runtime.rs
      command: cargo clippy --all-targets --all-features -- -D warnings
  - id: no-panic
    rule: Public functions do not panic; errors are propagated via Result.
    proof:
      kind: static-check
      target: src/agents
      command: cargo clippy --all-targets --all-features -- -D warnings
verification:
  pre_change:
    - cargo test --lib agents
  full:
    - cargo test
    - cargo clippy --all-targets --all-features -- -D warnings
---

# agents

## Architecture

The `agents` module is a thin I/O and formatting layer around AGENTS.md files.

- `parser.rs` owns YAML frontmatter parsing and the `AgentsManifest` / `AgentRole` data model.
- `runtime.rs` owns directory traversal (`load_project_agents`) and prompt-context injection (`inject_agents_context`).
- `mod.rs` is a storefront: it declares the two submodules and re-exports the two primary entry points.

Data flow: caller provides a starting directory → `load_project_agents` walks upward → `parser::load_agents_file` reads and parses the first AGENTS.md found → caller enriches prompts via `inject_agents_context`.

## Files

| File | Responsibility |
|------|----------------|
| `mod.rs` | Storefront: declares submodules, re-exports surface. |
| `parser.rs` | Parse AGENTS.md YAML frontmatter and body. |
| `runtime.rs` | Directory traversal, context injection, default template. |
