# Security Policy

## Supported Versions

OMK is pre-1.0 and ships from `master` only. The latest tagged GitHub release
is the supported version; older tags are not back-ported.

| Version               | Supported          |
| --------------------- | ------------------ |
| Latest tagged release | :white_check_mark: |
| Older tags            | :x:                |

## Reporting a Vulnerability

If you discover a security vulnerability in oh-my-kimi, please report it
responsibly:

1. **Do not open a public issue.**
2. Email **ekhodzitsky@gmail.com** with:
   - A description of the vulnerability
   - Steps to reproduce
   - Potential impact
   - Suggested fix (if any)

We acknowledge receipt within 48 hours and provide a timeline for a fix.

## Trust Boundaries

`omk` is a local orchestrator: it spawns external CLIs (Kimi, advisors,
gates) on behalf of a human operator and persists evidence on the local
filesystem. The trust model is intentionally narrow.

| Boundary | Trust level | Why |
| --- | --- | --- |
| Source code in this repository | Trusted | Reviewed by maintainers via PR. |
| `~/.config/omk/`, `~/.local/state/omk/`, `~/.local/share/omk/`, `~/.cache/omk/` (or the `$XDG_*` overrides) | Trusted | Owned by the human running `omk`; created with `0700`/`0600`. |
| `.omk/gates.toml` and other per-project config inside the working tree | Trusted | Lives in the repo you already trust; treat changes to it as you would treat any code change. |
| User-provided prompts, goals, task descriptions | **Untrusted** | Never spliced into shell strings, never expanded as paths. See "Untrusted Input Handling" below. |
| Agent (Kimi CLI) output and proposed tasks | **Untrusted** | Validated through goal task policy before any scheduling. |
| External advisor CLIs (`claude`, `codex`, `gemini`) | **Untrusted** | Spawned via argv (no shell interpreter); output is captured but not executed. |
| `MOCK_KIMI` env var | Trusted in dev/test, **must not be set in production** | Replaces the `kimi` binary path; documented as a test seam only. |
| `XDG_*` env vars | Trusted | Belong to the same user who runs `omk`. |
| `OMK_GOAL_INTERRUPT_POLL_MS`, `OMK_GOAL_AGENT_LEASE_SECS` | Trusted, numeric only | Parsed as bounded integers; invalid values fall back to defaults. |

## Threat Model

### Assets we protect

1. **Local secrets** in env vars, shell history, dotfiles, and `~/.ssh`.
2. **Wire / event log evidence** persisted under `~/.local/state/omk/` — must
   not leak credentials reviewers see by accident.
3. **The repository working tree** — must not be mutated outside the
   project root or in `.git`/`.github` metadata.
4. **CI tokens** in GitHub Actions — must not be readable by build steps
   that don't need them.

### Threats we mitigate

| Threat | Mitigation |
| --- | --- |
| Prompt-injected shell command via advisor or goal description | Advisors spawn via `Command::new(provider).arg("-p").arg(prompt)` (no `bash -c`). Gates and git use direct argv (`Command::new("git").args(...)`). The only remaining shell use is `shlex::try_quote` for human-displayed commands. |
| Path traversal in goal artifacts | `runtime::sanitize::sanitize_name`/`resolve_safe_path` reject `..`, `/`, `\`, `:`, leading dots, oversize names. Goal worktree branch/path components are normalized in `runtime/goal/worktree.rs`. |
| Agent proposes write to CI metadata or git internals | `is_safe_goal_agent_path` rejects any path component beginning with `.git` (covers `.git/`, `.gitignore`, `.gitmodules`, `.gitattributes`, `.github/workflows/...`, `.gitlab-ci.yml`) plus absolute paths, `~`, control characters, and parent-dir traversal. |
| Secret-scanner exfiltration via symlinked file | `runtime/goal/verifier.rs::scan_goal_security_findings` canonicalizes each candidate and skips any path that resolves outside the canonicalized project root. |
| Credentials leaking into wire / event logs | `wire::protocol::redact::redact_wire_secrets` redacts on both well-known keys (`api_key`, `token`, `authorization`, `*_secret`, …) and known value shapes (GitHub PATs, AWS access keys, Slack tokens, Stripe keys, Bearer headers, PEM private-key blocks). Redaction is idempotent. |
| Untrusted file permissions on state | `runtime/config::ensure_private_dir` chmods `0o700`; `runtime/atomic::atomic_write` chmods `0o600` (Unix). State paths default to `$XDG_STATE_HOME/omk/`. |
| Untrusted dependency or registry pulled in by a transitive crate | `deny.toml` denies unknown registries and unknown git sources, and runs in CI through `cargo-deny` on every PR. |
| GitHub Actions step exfiltrates `GITHUB_TOKEN` | All workflows declare an explicit `permissions:` block (least-privilege), and `actions/checkout` is invoked with `persist-credentials: false`. The release workflow scopes `contents: write` to the release job only; build jobs run with `contents: read`. |

### Threats we do **not** mitigate (known gaps)

- **Trojaned dependency at the registry source.** Once a dependency is
  published to crates.io, we have no way to detect malicious code in its
  source. We mitigate the blast radius through `cargo-deny` advisories, a
  pinned `Cargo.lock`, and code review of dependency bumps, but a
  compromised upstream is still a viable attack.
- **Compromised host running `omk`.** Local state is hardened to `0700`/`0600`
  but the user's shell/env is fully trusted. A compromised shell sees
  everything `omk` sees.
- **Mocked Kimi binaries.** Setting `MOCK_KIMI=/path/to/binary` lets a
  caller substitute an arbitrary executable for `kimi`. This is documented
  as a test-only seam; production deployments must not set it.
- **Out-of-band agent side effects.** Once Kimi receives a prompt, omk
  controls only the stated intent (`read_set`/`write_set`/`is_safe_goal_agent_path`),
  not the actual filesystem operations the agent performs. Defense lies in
  prompt boundaries (do-not-commit/do-not-publish reminders) plus the
  per-task mutation diff captured for review.

## Untrusted Input Handling

| Input source | Sink | Defense |
| --- | --- | --- |
| Goal text from CLI | Prompts to Kimi, persisted artifacts | Whitespace-normalized; never spliced into shell strings. |
| Agent task proposals | Scheduler tasks, prompts, write-set enforcement | `validate_goal_agent_task_proposals` rejects empty/dup ids, publishing intents, and any read/write path that fails `is_safe_goal_agent_path`. |
| Changed-file list from `git diff` | Security review scanner | `safe_project_file_path` rejects absolute / parent / prefix components; the scanner canonicalizes and refuses files that escape the project root. |
| Advisor prompts | `Command::new(provider).arg("-p").arg(prompt)` | Argv-mode invocation; no shell interpreter; provider name is gated against `ALL_PROVIDERS`. |
| Gates config (`.omk/gates.toml`) | `Command::new(gate.command).args(gate.args)` | Gate config is trusted (lives in the repo). Direct argv prevents shell injection through gate output. |

## Dependency Audit Policy

- `Cargo.lock` is committed and reviewed alongside any dependency bump.
- `deny.toml` enforces:
  - `advisories.yanked = "warn"` — yanked crates surface as warnings;
    `advisories.ignore` is small and annotated with the upstream reason.
  - `sources.unknown-registry = "deny"` and `sources.unknown-git = "deny"`
    — only crates.io is sanctioned.
  - `licenses.allow` is an allow-list (no `deny`/`unknown` license slips in
    by accident).
- CI runs `cargo deny --all-features check advisories licenses sources` on
  every PR via `.github/workflows/ci.yml`.

## GitHub Actions Posture

- Every workflow declares an explicit top-level `permissions:` block.
  Workflows that do not need write access pin to `contents: read`. The
  release workflow declares `contents: read` at the workflow level and
  scopes `contents: write` to the publish job only.
- Every `actions/checkout` invocation sets `persist-credentials: false`
  so the `GITHUB_TOKEN` is not left on disk for downstream steps to read.
- `cargo install cargo-tarpaulin` is pinned with `--locked --version ^0.31`
  so a tarpaulin compromise cannot trivially target our coverage step.
- Third-party actions are pinned to a major-version tag (`@v6`, `@v3`,
  …). Pinning to a commit SHA is a tracked follow-up; see "Known Gaps"
  below.

## Known Gaps and Follow-ups

- Third-party GitHub Actions are pinned by version tag, not commit SHA. A
  malicious force-push or compromised maintainer at the action repository
  would still be picked up. Track this as a future tightening pass.
- The wire value-pattern redactor is best-effort — a high-entropy random
  string that does not match the curated patterns will still be persisted.
  Operators who handle exotic token formats should add a redaction pattern
  in `src/wire/protocol/redact.rs` before persisting logs.
- `MOCK_KIMI` is honoured at runtime. There is no separate "release build"
  that disables the env-var test seam; production deployments must rely on
  operational hygiene instead.

## Security Considerations

- `omk` runs local commands and Kimi processes. Always validate input with
  `validate_safe()` before passing data to shell-command helpers.
- `omk` uses `shlex::try_quote` only for human-displayed command rendering.
  Process spawn uses argv (`Command::new(...).args(...)`) so a future
  regression in `shlex` cannot become an RCE.
- State files may contain sensitive task descriptions. Ensure
  `~/.local/state/omk/` has appropriate permissions (`0700`); `omk` enforces
  this on every write.
