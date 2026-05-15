---
schema_version: 1
module: skills
level: root
purpose: Discover, parse, and inject SKILL.md files with YAML frontmatter into prompts.
status: pilot
surface:
  - name: discovery
    kind: module
    visibility: pub
    contract: Re-exports skill discovery from the filesystem.
    proof:
      kind: unit-test
      target: skills::discovery
      command: cargo test --lib skills::discovery
  - name: injector
    kind: module
    visibility: pub
    contract: Re-exports skill injection and trigger matching.
    proof:
      kind: unit-test
      target: skills::injector
      command: cargo test --lib skills::injector
  - name: parser
    kind: module
    visibility: pub
    contract: Re-exports SKILL.md parsing and the Skill struct.
    proof:
      kind: unit-test
      target: skills::parser
      command: cargo test --lib skills::parser
  - name: Skill
    kind: struct
    visibility: pub
    contract: Parsed representation of a SKILL.md file with YAML frontmatter.
    proof:
      kind: unit-test
      target: skills::parser::tests::test_parse_frontmatter
      command: cargo test --lib skills::parser::tests::test_parse_frontmatter
  - name: parse_skill
    kind: fn
    visibility: pub
    contract: Parse a single SKILL.md file into a Skill struct. No panics; returns Err on missing or invalid frontmatter.
    proof:
      kind: unit-test
      target: skills::parser::tests::test_parse_frontmatter
      command: cargo test --lib skills::parser::tests::test_parse_frontmatter
  - name: discover_skills
    kind: fn
    visibility: pub
    contract: Discover skills from project, user, and bundled directories in priority order. Deduplicates by name.
    proof:
      kind: unit-test
      target: skills::discovery::tests::test_load_bundled_team_skill
      command: cargo test --lib skills::discovery::tests::test_load_bundled_team_skill
  - name: load_bundled_skill
    kind: fn
    visibility: pub
    contract: Load a bundled skill by name from CARGO_MANIFEST_DIR/skills/<name>/SKILL.md.
    proof:
      kind: unit-test
      target: skills::discovery::tests::test_load_bundled_team_skill
      command: cargo test --lib skills::discovery::tests::test_load_bundled_team_skill
  - name: find_skill
    kind: fn
    visibility: pub
    contract: Find a skill by name or alias in a skill slice. Case-insensitive.
    proof:
      kind: missing
      target: skills::discovery::find_skill
      command: ""
  - name: inject_skill
    kind: fn
    visibility: pub
    contract: Build a prompt with skill injection. Pure formatting; no I/O.
    proof:
      kind: missing
      target: skills::injector::inject_skill
      command: ""
  - name: match_trigger
    kind: fn
    visibility: pub
    contract: Check if a prompt matches any skill trigger substring. Case-insensitive.
    proof:
      kind: missing
      target: skills::injector::match_trigger
      command: ""
dependencies:
  internal:
    - module: runtime::config
      scope: discovery.rs only
      reason: Resolves the user-scope skills directory via data_dir().
  external:
    - name: anyhow
      scope: error propagation
      reason: Fallible parsing and I/O operations use Result and Context.
    - name: tokio
      scope: async filesystem I/O
      reason: Async file reads in parse_skill, discover_skills, and load_bundled_skill.
    - name: tracing
      scope: logging
      reason: Structured debug/info/warn logging during discovery and parsing.
    - name: regex
      scope: parser.rs only
      reason: Extract YAML frontmatter delimiter boundaries from SKILL.md content.
    - name: serde
      scope: parser.rs only
      reason: Deserialize YAML frontmatter into SkillMeta.
    - name: serde_yaml
      scope: parser.rs only
      reason: YAML parsing of SKILL.md frontmatter.
consumers:
  - path: "(none)"
    uses: ["Module is exported but currently unused by other src/ modules. cli/skill.rs manages skills independently."]
invariants:
  - id: storefront-mod-rs
    rule: mod.rs is a storefront under 100 lines with no business logic.
    proof:
      kind: static-check
      target: src/skills/mod.rs
      command: "wc -l src/skills/mod.rs"
  - id: no-super-super
    rule: "No super::super:: imports in the module."
    proof:
      kind: static-check
      target: src/skills/
      command: "grep -r 'super::super::' src/skills/ || echo none-found"
  - id: parse-no-panic
    rule: parse_skill never panics; returns Result on all error paths.
    proof:
      kind: unit-test
      target: skills::parser::tests::test_parse_frontmatter
      command: cargo test --lib skills::parser::tests::test_parse_frontmatter
  - id: discover-deduplicate
    rule: discover_skills deduplicates by skill name using a HashSet.
    proof:
      kind: static-check
      target: skills::discovery::discover_skills
      command: "grep -A2 'seen_names.insert' src/skills/discovery.rs"
  - id: inject-pure
    rule: inject_skill performs no I/O and has no side effects.
    proof:
      kind: static-check
      target: skills::injector::inject_skill
      command: "grep 'inject_skill' src/skills/injector.rs"
verification:
  pre_change:
    - cargo test --lib skills
  full:
    - cargo test
    - cargo clippy --all-targets --all-features -- -D warnings
---

# skills

## Architecture

```
┌─────────────────────────────────────────┐
│           src/skills/mod.rs             │  ← storefront (3 lines)
│  pub mod discovery; pub mod injector;   │
│          pub mod parser;                │
└─────────────┬─────────────┬─────────────┘
              │             │
    ┌─────────▼──────┐ ┌────▼──────┐ ┌────▼─────┐
    │   discovery    │ │ injector  │ │  parser  │
    │                │ │           │ │          │
    │ discover_skills│ │inject_skill│ │parse_skill│
    │ load_bundled_  │ │match_trigger│ │  Skill   │
    │    skill       │ │           │ │          │
    │   find_skill   │ │           │ │          │
    └────────────────┘ └───────────┘ └──────────┘
```

- **parser**: Owns the `Skill` data model and YAML frontmatter extraction.
- **discovery**: Owns filesystem scanning across project/user/bundled scope.
- **injector**: Owns pure prompt-formatting and trigger matching.

## Files

| File | Owns |
| --- | --- |
| `mod.rs` | Module exports only. 3-line storefront. |
| `parser.rs` | `Skill` struct, `parse_skill`, YAML frontmatter regex extraction. |
| `discovery.rs` | `discover_skills`, `load_bundled_skill`, `find_skill`, directory scanning. |
| `injector.rs` | `inject_skill`, `match_trigger`. Pure formatting, no I/O. |

## Edit Rules

- Keep `mod.rs` under 100 lines and free of business logic.
- Add new skill behavior to the appropriate submodule (`parser`, `discovery`, or `injector`).
- `parser.rs` owns the `Skill` struct; changes to fields must update `SkillMeta` and frontmatter extraction.
- Discovery uses `crate::runtime::config::data_dir()` for user-scope paths; do not hardcode `~/.omk/skills`.
- No `super::super::` imports. Use `super::` for sibling submodule access only.

## Known Gaps

- No internal crate consumers yet. `cli/skill.rs` manages skills independently via raw filesystem operations.
- `find_skill`, `inject_skill`, and `match_trigger` lack unit tests.
