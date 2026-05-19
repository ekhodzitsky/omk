# agents — Agent Guide

## Editing Rules

1. **Frontmatter delimiters are a contract.** The `---` opener/closer split in
   `parser.rs` is load-bearing. Changes need unit-test coverage for both
   frontmatter and no-frontmatter paths.
2. **Context injection is deterministic.** `inject_agents_context` must produce
   identical output for identical inputs. Do not add timestamps, random ordering,
   or non-stable formatting.
3. **Upward search terminates at root.** `load_project_agents` walks parent
   directories and must not loop or panic when reaching the filesystem root.
4. **No writes to AGENTS.md.** This module reads and parses manifests.
   `default_agents_md` returns a template string; the caller handles writes.
   Do not add file-writing logic here.
5. **Test with in-memory strings.** All parser and injection logic is tested
   without `tokio::fs` or temp files.
