# skills — Agent Guide

## Editing Rules

1. **Discovery is filesystem-only.** `skills::discovery` walks directories and
   reads `SKILL.md` files. No network calls, no writes.
2. **Injection is deterministic.** Given the same skill set and trigger context,
   `injector` must produce the same prompt augmentation. Sort skills by path
   to ensure stable ordering.
3. **YAML frontmatter is optional.** A skill without frontmatter is still valid.
   Do not require `description` or `triggers` fields.
4. **Paths are relative to the project root.** Store skill paths as
   `PathBuf` relative to the repo root so that rendered output is portable.
5. **Test through mocks.** `InMemorySkillSource` provides skill text and paths
   without touching `tokio::fs`. All trigger matching logic uses the mock.
