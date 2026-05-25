# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Auto-rebase with conflict recovery for goal slices**: `attempt_auto_rebase`
  now classifies merge conflicts as safe (whitespace, line-ending, comment-only)
  or unsafe (substantive code changes, deletions). Safe conflicts are
  auto-resolved and the rebase continues; unsafe conflicts abort the rebase
  and preserve detailed conflict evidence for manual resolution.
- `ConflictClassification` enum (`Safe` / `Unsafe`) added to
  `GoalMergeConflictEvidence` and exported from `omk::runtime::goal`.
- `GitRepo` gained `add`, `rebase_continue`, `conflicted_files`, and
  `status_porcelain` methods to support the new conflict resolution flow.
- **Release Discipline rules**: added `## Release Discipline (Hard Constraints)`
  section to `AGENTS.md` with per-PR CHANGELOG, documentation sync, version
  bump policy, release cut procedure, and backward-compat horizon rules.
- **PR template fields**: added `## Changelog` and `## Documentation` sections
  to `.github/pull_request_template.md` with checkboxes for CHANGELOG entries,
  version impact, and documentation updates.

- **Chat-first CLI surface**: running `omk` with no arguments opens a
  terminal-native chat REPL with a conversation log, engine pane, and
  autonomous escalation. The `omk chat` alias is also available.
- **Intent classifier**: routes user requests by size and complexity using a
  heuristic layer backed by Kimi for escalation decisions.
- **Conversation router and escalation bridge**: connects chat sessions to
  small-edit dispatch, medium plans, and full `omk goal` runs with observable
  autonomous-escalation events.
- **TUI pane rendering engine**: snapshot-tested terminal output for the engine
  pane with collapsed, compact, and expanded states.
- **Chat control surface**: slash commands, hotkeys, preflight keys, and theme
  switching between dark and light modes.
- **Autonomous-mode default**: the router no longer blocks on preflight prompts
  unless explicitly opted in; escalation decisions proceed with observable log
  markers instead of user input.
- **Session escalation log and TUI visibility marker**: autonomous decisions
  and large escalations are surfaced in the chat UI without blocking.

- **MCP client commands**: `omk mcp list`, `omk mcp doctor`, and
  `omk mcp call` with stdio and HTTP/SSE transport support and an LRU-cached
  client via `moka`.
- **ApprovalProxy**: configurable approval policy engine (`Never`, `Safe`,
  `Yolo`, `Pattern`) for autonomous gate and tool execution.
- **Wire hook integration**: native scripts placed in `.kimi/hooks/` execute
  automatically on hook requests with timeout enforcement and event emission.

- **LLM planner in goal CLI**: goal planning now uses an LLM planner by default
  with graceful fallback to a heuristic planner when the LLM is unreachable.
- **Slice PR delivery hardening**: auto-rebase, proof validation, and conflict
  detection before recording delivery evidence.
- **Six-review wall**: architect, code, test, security, performance, and
  anti-slop review passes now run before a slice PR can be opened.
- **Anti-slop heuristics**: real rough-edge detection in changed files; the
  controller can auto-spawn follow-up refactor tasks from review findings.
- **Aggregate review verdict + auto-merge**: slice PRs automatically merge when
  all six review passes are green and required CI checks succeed.
- **Auto-rebase on merge-tree conflict**: rebases slice branches before
  recording evidence when merge-tree detects conflicts.
- **Concurrent slice ownership leases**: per-slice conflict detection with
  lease metadata and stale-worker cleanup.

- **Security redaction at all boundaries**: secrets are scrubbed from gate
  stdout/stderr, proof artifacts, event streams, and CLI JSON output.
- **Logging path hardening**: centralized redaction prevents secret leakage
  through tracing spans and log files.
- **Security cleanup tasks**: verifier findings can auto-spawn dedicated
  security cleanup tasks.

- **Tree-sitter code analysis**: AST-based structural understanding for Rust,
  JavaScript/TypeScript, Python, and Go.
- **Exact token counting**: `tiktoken-rs` integration for accurate BPE token
  counts and USD cost estimation.

- **SQLite storage module**: durable goals, tasks, events, proofs, budget
  checkpoints, and artifacts stored in SQLite with WAL mode and migration
  versioning.
- **Goal event persistence in SQLite**: full roundtrip integration tests for
  event storage and retrieval.
- **Typed GitRepo abstraction**: replaces raw git command strings with a
  structured API.

- **Performance/scale hardening**: Criterion benchmarks and cycle detection
  optimization for goal runtime operations.
- **Parallel post-processing**: structured concurrency and parallel processing
  for goal runtime operations.
- **tokio-console feature flag**: enables async runtime observability via the
  Tokio console subscriber.

### Changed

- **README rewritten** for autonomous-agent positioning; chat-first surface is
  now the documented default entry point.
- **Cargo.toml metadata trimmed** for crates.io readiness.
- **Direct dependencies trimmed** to reduce compile times and supply-chain
  surface.

### Fixed

- Eliminated `kill_on_drop` race condition causing `exit_code=None` on Linux
  during gate execution.
- Preserved hook results when scripts close stdin early.
- Fixed MCP process leak, transport demux, and server-scoped call routing.

### Wire Protocol

- Added `ContentPart` variant to the `Event` enum so it matches the documented
  wire event types table.
- Documented that `Event::ToolCall` serializes as `"function"` on the wire.
- Added unknown/extra field tolerance tests for `Event`, `Request`, and
  `InitializeResult`.
- Added golden round-trip tests for protocol message fixtures.
- Added end-to-end redaction test for `ToolCall` secrets.

#### Wire Protocol Version History

| Version | Introduced In | Notes |
|---------|---------------|-------|
| 1.9 | 0.4.0 | Current version; observed from Kimi Code CLI 1.41.0. |
| 1.7 | 0.2.0 | Added `set_plan_mode` / `steer` control methods. |
| 1.0 | 0.1.0 | Initial wire protocol support. |

### Dependencies

- SQLite stack: `rusqlite` upgraded 0.30→0.37, `tokio-rusqlite` 0.5→0.7.
- `tiktoken-rs` upgraded 0.6→0.11.
- Tree-sitter parent upgraded 0.25→0.26; grammars updated: `tree-sitter-rust` 0.23→0.24, `tree-sitter-go` 0.23→0.25, `tree-sitter-python` 0.23→0.25, `tree-sitter-javascript` 0.23→0.25.
- `shlex` upgraded 1.3.0→2.0.1.
- `console-subscriber` upgraded 0.4→0.5.
- GitHub Actions bumps: `actions/checkout` 4.2.2→6.0.2, `actions/upload-artifact` 4.6.2→7.0.1, `actions/download-artifact` 4.2.1→8.0.1, `actions/attest-build-provenance` 2.2.3→4.1.0, `actions/stale` 9.1.0→10.3.0, `Swatinem/rust-cache`, `taiki-e/install-action` 2.50.0→2.79.4, `EmbarkStudios/cargo-deny-action` 2.0.18→2.0.19, `codecov/codecov-action`.

### Documentation

- Restructured `AGENTS.md` with architecture, testing, security, and
  observability sections.
- Added manual recovery guide for failed PR, CI, review blockers, merge
  conflicts, and partial acceptance.
- Added recovery docs and CLI recovery hints.
- Restructured `TODO.md` and updated `ROADMAP.md` Stage 5.
- Completed audit follow-up for `API.md`, `AGENTS.md`, `CONTRIBUTING.md`, and
  `KIMI_UPSTREAM.md`.
- Refreshed `README.md` for conciseness and accuracy.
- Updated README, API reference, project map, and tutorials to
  cover chat REPL, MCP client commands, goal budget/merge
  subcommands, and additional goal execution flags shipped
  since 0.4.0.

## [0.4.0] - 2026-05-13

### Added

- **`omk goal` delivery metadata API**: task delivery sidecars now have typed
  Rust helpers for owner, write scope, branch, worktree, PR, commit,
  verification summary, and status metadata while preserving unknown legacy
  JSON fields for existing task graphs and proof artifacts.
- **`omk goal` task-scoped worktree delivery**: worktree materialization now
  creates deterministic task branches/worktree paths and records task owner,
  write scope, branch, worktree, and planned delivery status back into the task
  graph for PR/proof rendering.
- **`omk goal` domain types**: goal creation now uses dedicated `GoalId` and
  `GoalBudget` wrappers, while `GoalKind` exposes stable machine strings for
  oracle-facing code without changing serialized goal-state compatibility.
- **Structured `omk goal` review wall**: goal review proof output now surfaces
  deterministic architect, code, test, security, performance, and anti-slop
  review sections with status, evidence, risks, known gaps, and a recommended
  next step for PR readiness.
- **`omk goal` crash recovery and replay hardening**: `replay_goal` is now
  idempotent—duplicate exact JSON events are deterministically collapsed,
  partial/corrupt trailing event lines are surfaced as `known_gaps` with a
  `recovery_status`, and missing optional artifacts (task graph, proof) become
  recoverable gaps instead of hard errors. `GoalState::load` returns typed
  `GoalStateError` variants (`MissingFile`, `CorruptedJson`, `InvalidFormat`)
  so callers can distinguish missing state from malformed JSON. `GoalProof`
  carries an optional `recovery_status` field that is populated when proof is
  rebuilt from state after a missing or unreadable proof file.
- **`omk goal open-pr` dry-run output**: goals with proof evidence can now render
  GitHub PR title/body drafts as Markdown, JSON, or text without network access
  or GitHub auth; scaffold-only proofs fail with an actionable next step.
- **`omk goal` local integrator readiness**: `omk goal accept` / `reject`
  record explicit integration decisions. `ready` now requires gates, bounded
  execution, review wall, oracle evidence, and local integrator acceptance; PR
  drafts include review, integration, and oracle evidence.
- **`omk goal` terminal proof status hardening**: `goal proof` now reconciles
  stale or rebuilt proof output with durable goal state for `blocked_on_human`,
  `needs_more_budget`, `cancelled`, `blocked_on_external`, and `failed_infra`
  outcomes, with regression coverage for each proof status family.
- **`omk goal` merge-conflict delivery evidence**: goal worktree delivery can
  run a read-only `git merge-tree` conflict check for task branches, write a
  merge-conflict artifact, and mark task delivery metadata as blocked or ready
  for review without mutating branches.
- **`omk goal` notification extension point**: documented the no-dependency
  watcher contract for goal state, event, proof, failure, and budget files so
  operators can attach local notifications without implicit network side
  effects.
- **`omk goal open-pr --draft` rendering**: dry-run PR output can now mark the
  generated PR metadata/body as a GitHub draft while preserving the no-network
  creation guard, and renders canonical task `pr_url` delivery metadata while
  preserving the older `pr_link` alias.
- **`omk goal` release-candidate PR notes**: generated PR drafts now include a
  release-candidate section with draft state, no-mutation disclosure, and a
  merge recommendation derived from proof status and known gaps.
- **`omk goal` planning oracle criteria**: generated `test-spec.md` now records
  the classified oracle kind and required checks for greenfield and rewrite
  planning fixtures before execution begins.
- **`omk goal` rewrite oracle runner**: rewrite oracle support now includes a
  timeout-backed command observation runner that captures stdout, stderr, exit
  code, and declared file artifacts for compatibility comparison.
- **`omk goal` intentional rewrite incompatibilities**: rewrite compatibility
  comparisons can now allowlist deliberate stdout, stderr, exit-code, or file
  artifact differences while keeping unexplained mismatches blocking.
- **`omk goal` source surface detection**: rewrite oracle support now detects
  Rust, Node, and Python command/API surfaces from local project files without
  executing commands.
- **`omk goal` rewrite compatibility planning**: rewrite/refactor/migration
  `test-spec.md` output now includes a compatibility test plan with detected
  source commands and API/file surfaces.
- **`omk goal` rewrite golden capture**: rewrite oracle observations can now be
  written into the stable fixture layout used by compatibility golden tests.
- **`omk goal` Python-to-Rust rewrite demo**: rewrite oracle fixtures now include
  a tiny Python CLI and equivalent Rust CLI with matching behavior snapshots.
- **`omk goal` readiness-level planning**: generated goal test specs now
  distinguish proof-backed engineering readiness from broader product-release
  readiness.
- **`omk goal` greenfield oracle artifacts**: greenfield planning now emits
  local acceptance, smoke/demo, and docs-first usage artifacts under
  `artifacts/oracles/`.
- **`omk goal` rejection rollback plans**: local integrator rejection now writes
  a rollback-plan artifact that preserves the rejected reason and changed-file
  scope before the proof is finalized as `not_ready`.
- **`omk goal` gated merge policy**: `--merge-policy gated` polls required CI
  checks on the opened PR and auto-merges after they pass; `manual` blocks on
  human decision; `disabled` opens the PR without merge.
- **`omk goal` per-slice execution in worktrees**: `--slice-execution` runs each
  agent task in an isolated git worktree with a deterministic branch, serializes
  overlapping write scopes, and cleans up worktrees on successful delivery.
- **`omk goal` per-slice PRs and review/fix loop**: when slice execution is
  combined with `--policy draft-pr|auto-pr`, each slice is auto-committed,
  pushed, and opened as a dedicated PR; a per-slice review runs gates and
  security scan; failed slices are reset to `Pending` with review feedback
  injected into the next agent prompt for automatic retry.
- **`omk goal` integrator PR**: after all slices are `Delivered`, the controller
  creates an `integrator/{goal-id}` branch from current master, merges all slice
  branches into it, pushes, and opens an integrator PR that follows the chosen
  `merge_policy`.
- **`omk goal` controller narrative**: `run_goal_until_ready` now emits
  `TaskOutput` events after each controller step; the CLI renders a numbered
  `Narrative:` section with emoji icons for plan, verify, execute, review,
  deliver, and blocked steps.

### Changed

- **Worktree/PR-first development workflow**: Beads is no longer required for
  multi-agent development or `omk goal` delivery. The canonical coordination
  path is now task-scoped worktrees/branches, explicit write scopes, PR
  evidence, green CI, and review; external trackers remain optional.
- **CI feedback lanes**: PR checks now use a faster Ubuntu gate plus macOS
  smoke compatibility, while docs, full macOS build/test, coverage upload, and
  release artifacts stay on protected-branch, scheduled, manual, or release
  workflows.
- **Documentation audit**: refreshed README, SPEC, ROADMAP, TODO, CONTRIBUTING,
  AGENTS, SECURITY, and `docs/*` so the worktree/PR workflow, role names, MVP
  status, and goal-runtime feature surface are consistent across files.
  Replaced run-on goal-scaffold paragraphs with terse bullet lists pointing at
  `SPEC.md` / `TODO.md` for the canonical surface.
- **`omk goal` MVP docs**: tutorial, troubleshooting, architecture, competitive
  positioning, and README feature-table docs now cover the goal MVP flow,
  blocked/rejected/budget recovery, task-scoped delivery metadata, and
  greenfield/rewrite/audit/refactor examples.

### Fixed

- **`VERSION` file synced with `Cargo.toml`**: bumped `VERSION` from `0.3.1` to
  the actual crate version `0.3.30`.
- **Stale role name `coder`**: renamed remaining `coder` references in
  `docs/PROJECT_MAP.md` and `docs/north_star_tutorial.md` to the canonical
  `executor` role used by the role pack and CLI.
- **Hardcoded version string in `docs/API.md`**: replaced the static `0.3.4`
  health response example with a `<crate-version>` placeholder that tracks the
  running binary.
- **`SECURITY.md` supported versions**: collapsed obsolete 0.1.x / 0.2.x rows
  into a single "latest 0.3.x" row, matching the pre-1.0 master-only release
  policy.

## [0.3.30] - 2026-05-13

### Added

- **Multi-agent Beads/PR workflow**: documented Beads as the durable coordination layer for Codex/Kimi/Claude/human collaboration, locked `master`/`main` as read-only baselines, refreshed the PR template with bead/write-scope/verification fields, and added goal roadmap/spec items for bead-backed PR delivery.

### Changed

- **Generated docs ignore rules**: `.gitignore` now excludes generated documentation/site outputs and coverage reports while keeping source documentation tracked.
- **Beads bootstrap docs**: contributor guidance now uses the current `bd 1.0` initialization flags and keeps Beads-generated agent docs/hooks out of the repo-managed workflow.

## [0.3.29] - 2026-05-13

### Added

- **`omk goal` human-blocked oracle guard**: vague goals without testable success criteria now stop as `blocked_on_human`, write `failure.json`, include the required human decision in `proof.json`, and prevent `verify`/`execute`/`review` from continuing until the goal is refined.

### Fixed

- **Event reader test determinism**: raw JSONL reader tests now flush manual async file writes before reading summaries, matching the production writer contract and keeping macOS CI deterministic.

## [0.3.28] - 2026-05-13

### Added

- **`omk goal` task retry/lease metadata**: durable `task-graph.json` nodes now carry `retry_count`, `max_retries`, and `lease_expires_at` fields, load older graphs with safe defaults, and increment retry evidence when a Wire-backed goal task wave blocks.

## [0.3.27] - 2026-05-13

### Fixed

- **Goal state backward-compatible loading**: `GoalState::load` now accepts legacy `goal.json` files that predate `until_ready`, `terminal_criteria`, `phase`, `version`, artifacts, and `state_dir`, and it rehomes stale persisted `state_dir` values to the actual goal directory so moved/restored state stores keep working.

## [0.3.26] - 2026-05-13

### Added

- **`omk goal` decision log**: goal run/plan scaffolds now write a durable `decisions.jsonl` artifact with controller-owned planning, task graph, and execution-boundary decisions, making goal rationale inspectable alongside `events.jsonl`, `task-graph.json`, and `proof.json`.

## [0.3.25] - 2026-05-13

### Added

- **Stale worker cleanup**: scheduler runs now quarantine workers that lose an expired task lease, write a durable `stale-worker-cleanup.json` marker, emit `worker_dead` evidence, ignore later stale-worker outbox/heartbeat updates, and fail fast instead of hanging when no live workers remain.

## [0.3.24] - 2026-05-13

### Added

- **Token/cost goal budget hard stops**: `omk goal run` now accepts `--budget-tokens` and `--budget-usd`, goal budget reports include Wire-derived token usage plus estimated USD cost, and `verify`/`execute`/`review` stop with `needs_more_budget` when token or cost budgets are exhausted.
- **Token/cost budget recovery**: `omk goal budget-add` now accepts `--tokens` and `--usd` in addition to `--time`, extending exhausted budgets relative to already observed usage so operators can safely resume token/cost-blocked goals.

## [0.3.23] - 2026-05-13

### Added

- **Per-task Wire budget hard stops**: scheduler-dispatched worker tasks now carry structured `budget_secs`, and Wire workers enforce that budget as a task timeout with failed-result/event evidence instead of treating per-task budgets as prompt-only guidance.

## [0.3.22] - 2026-05-13

### Fixed

- **Verification gate output draining**: timeout-bounded gates now use process output collection that drains stdout/stderr while waiting for process exit, preventing large-output gates from being killed by the timeout path and losing their real exit code/evidence artifact metadata.

## [0.3.21] - 2026-05-13

### Added

- **Deterministic `omk goal replay` output**: goal replay now derives `generated_at` from persisted goal event evidence instead of the current process clock, making repeated replay JSON stable across separate CLI invocations for crash-recovery inspection.

## [0.3.20] - 2026-05-13

### Added

- **`omk goal` active pause interruption**: `goal execute` now watches durable goal state during Wire-backed agent waves, cancels active workers when an operator pauses or cancels the goal, preserves the interrupted goal/proof status, and prevents the scheduler from dispatching additional tasks after the interrupt.

## [0.3.19] - 2026-05-13

### Added

- **`omk goal budget-add` recovery path**: operators can now add wall-clock budget to a goal, move `needs_more_budget` goals back to `not_ready`, persist `budget_extended` checkpoints, and emit `goal_budget_extended` events so budget-stop recovery is durable and replayable.

## [0.3.18] - 2026-05-13

### Added

- **`omk goal` wall-clock budget enforcement**: goals with exhausted `--budget-time` now stop `verify`, `execute`, and `review` before spending more gates or agent work, persist `needs_more_budget`, append `budget_exhausted` checkpoints, and emit `goal_budget_exhausted` timeline events.

## [0.3.17] - 2026-05-13

### Added

- **`omk goal` budget checkpoints**: goals now write durable `budget-checkpoints.jsonl` entries, emit `budget_checkpoint` events, and expose `omk goal budget` as text, Markdown, or JSON so long-running budget state is inspectable after process restarts.

## [0.3.16] - 2026-05-13

### Added

- **`omk goal replay` timeline reconstruction**: goals can now replay their persisted `events.jsonl` plus task graph summary as text, Markdown, or JSON, making long-running goal history inspectable after separate CLI invocations and process restarts.

## [0.3.15] - 2026-05-13

### Added

- **`omk goal` pause/resume lifecycle**: goals can now be persisted as `paused`, resumed back to `not_ready`, emit `goal_paused`/`goal_resumed` events, and block `verify`/`execute`/`review` while paused until explicitly resumed.

## [0.3.14] - 2026-05-13

### Added

- **`omk goal` read/write access policy**: agent-proposed follow-up task validation now rejects unordered read/write and write/read path conflicts, while preserving dependency-ordered read-after-write tasks and parallel shared-read tasks.

## [0.3.13] - 2026-05-13

### Added

- **`omk goal` path-normalized write-set policy**: agent-proposed write-set conflict checks now normalize safe relative paths and reject unordered parent/child path overlaps, such as `./README.md` vs `README.md` and `docs` vs `docs/guide.md`.

## [0.3.12] - 2026-05-13

### Added

- **`omk goal` write-set conflict policy**: agent-proposed follow-up tasks that write the same path now require dependency ordering. The controller accepts dependency-serialized mutations, rejects unordered conflicts with `task_rejected` evidence, and avoids appending unsafe graph nodes.

## [0.3.11] - 2026-05-13

### Added

- **`omk goal` graph mutation events**: accepted agent-proposed task graph additions now emit first-class `task_graph_mutated` events with the task id, source, artifact paths, and resulting task count, making durable graph changes auditable beyond proposal/acceptance decisions.

## [0.3.10] - 2026-05-13

### Added

- **`omk goal` task graph validation**: goal task graphs are now validated on load for duplicate task ids, missing dependencies, self-dependencies, empty required task fields, and dependency cycles before controller execution proceeds.

## [0.3.9] - 2026-05-13

### Added

- **`omk goal execute` stale-task recovery**: goal agent waves now recover expired scheduler leases, emit `retry_scheduled` evidence with the stale worker id, and prefer a different available worker for the recovered task instead of immediately redispatching to the same stalled worker. Mock Wire tests can now target a single worker with `MOCK_KIMI_WIRE_STALL_WHEN_CONTAINS`.

## [0.3.8] - 2026-05-12

### Added

- **`omk goal execute` max-agent worker pools**: accepted ready goal-agent tasks now run through a bounded Wire worker pool capped by `--max-agents` and by the number of accepted ready tasks. Follow-up waves can fan out safely across multiple workers while keeping worker-0 evidence paths stable for existing proof readers.

### Changed

- Refactored `runtime::goal` into a module directory for SRP and AGENTS.md 400-line compliance. No behavior change.

### Fixed

- Replaced `std::sync::Mutex` with `tokio::sync::Mutex` in `runtime::watchdog` to eliminate executor-blocking in async context. Removed associated `.unwrap()` calls.
- Added `tokio::time::timeout` guards to `git` `Command::output().await` calls in `runtime::goal::evidence` to prevent indefinite hangs.
- Removed production `.unwrap()` calls from `runtime::worker` and `runtime::scheduler::claim`.
- Removed production `.expect()` calls from `cli::app` and `vis::server` signal/metrics handlers; errors now log gracefully instead of panicking.
- Refactored `notifications::webhook` into a module directory for AGENTS.md 400-line compliance. No behavior change.
- Refactored `cli::kimi_native_cmd` into a module directory for AGENTS.md 400-line compliance. No behavior change.
- Added `tokio::time::timeout` guards to `Command::output().await` and `Command::status().await` calls across `runtime/` (`ask`, `ralph`, `gates`, `retry`, `ultrawork`, `autopilot`) and `cli/` (`app`, `backup`, `doctor`, `logs`, `skill`) to prevent indefinite hangs from rogue child processes.
- Verified `runtime::ask` and `runtime::gates::run` `Command::spawn()` calls already carry `.kill_on_drop(true)` — no zombie-process risk.

## [0.3.7] - 2026-05-12

### Added

- **`omk goal execute` follow-up dispatch**: accepted agent-proposed tasks no longer just sit in `task-graph.json`. A later `goal execute` now selects ready pending executor-owned follow-up tasks, runs them through a separate `goal-agent-followups` Wire wave, records worker/run evidence, and marks the durable graph nodes done or blocked from actual worker results.

## [0.3.6] - 2026-05-12

### Added

- **`omk goal execute` agent-proposed tasks**: Wire workers can now return structured `OMK_TASK_PROPOSAL: {...}` follow-up work. The goal controller extracts those proposals, validates them through the same policy/budget/path checks, writes `agent-task-proposals.json`, emits proposal/decision events, and appends accepted safe follow-up tasks to `task-graph.json` as pending graph nodes instead of letting agents mutate the graph directly.

## [0.3.5] - 2026-05-12

### Added

- **`omk goal execute` controller loop**: the bounded agent wave is now a policy-validated multi-task dispatch instead of one scheduler task. The controller records `task-policy.json`, assigns per-task budgets and acceptance criteria, emits `task_proposed`, `task_accepted`, and `task_rejected` events, and rejects crates.io publishing in the current GitHub-only release lane.

## [0.3.4] - 2026-05-12

### Added

- **`omk goal execute` post-mutation gates**: when the bounded Wire-backed agent wave changes project files, `execute` now reruns verification gates against the mutated tree, records the post-mutation gate evidence, marks `post_mutation_gates_ran` in `proof.json`, and removes the stale-gates gap while still keeping integration/review readiness honest.

## [0.3.3] - 2026-05-12

### Added

- **`omk goal execute` mutation evidence**: the bounded Wire-backed agent wave can now make minimal project changes, captures `mutation.diff` and `changed-files.json` under `artifacts/agent-runs/goal-agent-execute/`, includes untracked files in changed-file detection, and keeps proofs `not_ready` until gates/review/integration rerun after agent mutations.

## [0.3.2] - 2026-05-12

### Added

- **`omk goal review` evidence pass**: added `omk goal review [goal-id|latest]`, explicit `goal-review` and `goal-security-review` task graph nodes, review artifacts under `artifacts/reviews/`, and a bounded high-confidence secret scan over changed files. Proofs now move past the review/security gap only after this pass, while still staying `not_ready` until the project mutation/integration loop exists.
- **`omk goal execute` bounded agent wave**: `execute` now runs a scheduler-backed `goal-agent-execute` task through the existing Wire worker adapter, records `artifacts/agent-runs/goal-agent-execute/` evidence, blocks quickly when Kimi is unavailable, and keeps proofs `not_ready` until review/security evidence exists.
- **`omk goal verify` proof wall**: added local verification gate execution for goals, full gate output artifacts under `artifacts/gates/`, gate events, changed-file capture, and proof refreshes that stay `not_ready` until execution and review evidence exists.
- **`omk goal execute` local controller step**: added `omk goal execute [goal-id|latest]`, split the placeholder execution task into `goal-local-verify` and `goal-agent-execute`, and now record local verification task evidence when required gates pass before the bounded agent wave runs.
- **`omk goal` git proof evidence**: goal proofs now capture best-effort git branch, HEAD commit, dirty state, and keep the current HEAD in the compatibility `commits` list when run inside a git worktree.
- **`omk goal` task evidence**: task graphs now mark controller-owned planning work as `done`, record owner/evidence metadata, and append task events from the `goal-controller` actor before local verification and bounded agent execution steps run.
- **`omk goal` controller scaffold**: `run` and new `plan` now write `prd.md`, `technical-plan.md`, `test-spec.md`, `task-graph.json`, and an honest `proof.json`; new `omk goal proof` renders the goal proof as text, JSON, or Markdown.
- **`omk goal` state scaffold**: added durable `goals/<goal-id>/goal.json` state under the OMK state directory, goal event logs, `run/list/status/show/cancel` CLI commands, JSON/Markdown/text output, and cancellation `failure.json` artifacts.

### Changed

- **Project coding contract**: added root `AGENTS.md` rules for library-first CLI structure, Wire protocol tests, machine-readable output boundaries, worker lifecycle semantics, deterministic Rust tests, dependency discipline, and release hygiene.
- **Runtime/module refactor**: split large team CLI, autopilot, event, proof, scheduler runner, Wire client, and Wire protocol modules into focused submodules without changing the public command surface.
- **GitHub project metadata**: refreshed README badges and repository metadata to emphasize the current Wire-first beta MVP, GitHub-only install path, and unpublished crates.io status.
- **`omk goal` product direction**: added the canonical goal spec, roadmap, backlog, and detailed design for long-running proof-backed autonomous engineering runs.
- **Competitive positioning**: documented the `omk goal` market map, direct and adjacent competitors, and the must-have "local, repo-native, proof-driven autonomous software engineering runtime" positioning.

## [0.3.1] - 2026-05-11

### Changed

- **Team CLI refactor**: split proof/failure-artifact writing and Wire worker run helpers out of the large `team` CLI module, keeping the CLI command surface focused on argument handling and user output.
- **Library-first CLI architecture**: moved the top-level Clap app and binary-owned command dispatch into the library crate, leaving `src/main.rs` as a thin `omk::cli::run()` wrapper and making the CLI entrypoint importable from integration tests.
- **Library-owned shutdown handling**: kept SIGINT/SIGTERM cancellation behind `omk::cli::run()`, so the binary stays a thin wrapper while team runs can stop Wire worker adapters through the library runtime.
- **Run metrics naming**: renamed current runtime metrics from spawn-oriented `total_spawns` to `total_team_runs`, while preserving `total_spawns` as a JSON/API and Prometheus compatibility alias.
- **Documentation accuracy**: refreshed README positioning, web API examples, and project/CLI maps to match the v0.3.1 Wire-first team layout.

## [0.3.0] - 2026-05-11

### Removed

- **Tmux team runtime**: removed the tmux-backed `omk team spawn`, `attach`, and `broadcast` command surface, the `omk hud --tmux` statusline output, tmux doctor checks, tmux package dependencies, and the old `runtime/tmux.rs` and `runtime/bridge.rs` modules. `omk team run` is now the single team execution path.

### Changed

- **Wire-first team contract**: team help, watchdog health, HUD output, MCP tools, packaging, README, tutorial, troubleshooting, API, and architecture docs now describe scheduler-backed Kimi Wire execution instead of terminal-pane orchestration. The MCP team tool is now `omk_team_run`.
- **GitHub-only public docs positioning**: README now documents why OMK exists, current MVP readiness, GitHub-only installation, usable features, limits, and how OMK compares with raw Kimi CLI, ad hoc scripts, and cloud orchestrators. Tutorial and project-map docs now align with current `team run`, HUD, and proof command shapes.

## Historical Archive

Release notes for 0.2.5 and earlier moved to [docs/CHANGELOG_ARCHIVE.md](docs/CHANGELOG_ARCHIVE.md) to keep the active changelog reviewable.
