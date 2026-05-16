//! `--help` / `after_help` strings for every `omk goal` subcommand.
//!
//! Kept separate from the `clap` derive so the prose can be reviewed and edited
//! without churning the module that owns parsing or dispatch. Examples avoid
//! shell-specific line continuations (`\`) so they render the same way on
//! POSIX shells, PowerShell, and Windows `cmd.exe`.

pub(super) const GOAL_TOP_AFTER_HELP: &str = "\
Examples:
  omk goal run \"Fix all clippy warnings\"
  omk goal run \"Rewrite project in Rust\" --until-ready --budget-time 8h
  omk goal list
  omk goal show latest --json
  omk goal verify latest
  omk goal execute latest
  omk goal review latest
  omk goal accept latest --summary \"local integrator accepted the proof\"
  omk goal reject latest --reason \"manual review found a blocker\"
  omk goal open-pr latest --dry-run

Goal state is stored under the OMK state directory, one folder per goal:
  <state-dir>/goals/<goal-id>/
    goal.json          -- durable state (status, phase, budgets)
    prd.md             -- goal brief
    technical-plan.md  -- controller plan
    task-graph.json    -- task graph with retries/leases
    proof.json         -- current proof bundle
    events.jsonl       -- append-only event log
    decisions.jsonl    -- controller decision log

Most commands accept either a concrete goal id or the alias `latest`.";

pub(super) const GOAL_RUN_AFTER_HELP: &str = "\
Examples:
  omk goal run \"Fix all failing cargo tests\"
  omk goal run \"Ship CLI UX polish PR\" --until-ready
  omk goal run \"Migrate Python to Rust\" --until-ready --budget-time 7d --budget-tokens 2000000 --budget-usd 25 --max-agents 3

Without --until-ready, the command creates a durable scaffold for inspection.
With --until-ready, it becomes the one-command controller: plan -> verify -> execute -> review.
Inspection commands remain available for manual recovery, but they are not the
happy-path driver.";

pub(super) const GOAL_LIST_AFTER_HELP: &str = "\
Examples:
  omk goal list";

pub(super) const GOAL_STATUS_AFTER_HELP: &str = "\
Examples:
  omk goal status
  omk goal status latest
  omk goal status goal-20260514-085416-149-ea263039";

pub(super) const GOAL_SHOW_AFTER_HELP: &str = "\
Examples:
  omk goal show
  omk goal show latest --json
  omk goal show latest --format md > GOAL.md";

pub(super) const GOAL_PROOF_AFTER_HELP: &str = "\
Examples:
  omk goal proof
  omk goal proof latest --json
  omk goal proof latest --format md";

pub(super) const GOAL_OPEN_PR_AFTER_HELP: &str = "\
Examples:
  omk goal open-pr latest --dry-run
  omk goal open-pr latest --dry-run --draft
  omk goal open-pr latest --dry-run --format json
  omk goal open-pr goal-20260514-085416-149-ea263039 --dry-run --format md
  omk goal open-pr latest --policy auto-pr --dry-run
  omk goal open-pr latest --policy draft-pr --base-branch main

Renders a local PR title/body draft from persisted goal proof evidence. Use
--policy local (default) to render without network mutation. Use --policy
auto-pr or --policy draft-pr to create or update a real GitHub PR via the gh
CLI. Use --draft to mark the rendered PR metadata as a draft.";

pub(super) const GOAL_REPLAY_AFTER_HELP: &str = "\
Examples:
  omk goal replay
  omk goal replay latest --json
  omk goal replay latest --format md";

pub(super) const GOAL_BUDGET_AFTER_HELP: &str = "\
Examples:
  omk goal budget
  omk goal budget latest --json";

pub(super) const GOAL_BUDGET_ADD_AFTER_HELP: &str = "\
Examples:
  omk goal budget-add --time 1h
  omk goal budget-add latest --tokens 500000
  omk goal budget-add latest --time 30m --usd 5

At least one of --time / --tokens / --usd must be provided.";

pub(super) const GOAL_VERIFY_AFTER_HELP: &str = "\
Examples:
  omk goal verify
  omk goal verify latest

Runs the configured local verification gates (cargo fmt, check, clippy, test,
doc by default) and writes the result into the goal proof.";

pub(super) const GOAL_EXECUTE_AFTER_HELP: &str = "\
Examples:
  omk goal execute
  omk goal execute latest";

pub(super) const GOAL_REVIEW_AFTER_HELP: &str = "\
Examples:
  omk goal review
  omk goal review latest";

pub(super) const GOAL_ACCEPT_AFTER_HELP: &str = "\
Examples:
  omk goal accept latest --summary \"local integrator accepted the proof\"

Marks a goal ready only when gates, execution, review wall, oracle evidence,
and explicit local integration acceptance are all present.";

pub(super) const GOAL_REJECT_AFTER_HELP: &str = "\
Examples:
  omk goal reject latest --reason \"manual review found a blocker\"

Records an explicit local integration rejection and keeps the proof not_ready.";

pub(super) const GOAL_PAUSE_AFTER_HELP: &str = "\
Examples:
  omk goal pause
  omk goal pause latest";

pub(super) const GOAL_RESUME_AFTER_HELP: &str = "\
Examples:
  omk goal resume
  omk goal resume latest";

pub(super) const GOAL_CANCEL_AFTER_HELP: &str = "\
Examples:
  omk goal cancel
  omk goal cancel latest

Records a `failure.json` artifact and stops further execution.";

pub(super) const GOAL_PLAN_AFTER_HELP: &str = "\
Examples:
  omk goal plan \"Investigate flaky verifier tests\"";

pub(super) const GOAL_LONG_ABOUT: &str = "\
Goal runtime -- durable, proof-driven engineering goals.

Each goal owns a state directory with PRD, technical plan, task graph,
event log, and proof bundle. `omk goal run --until-ready` drives the primary
one-command controller loop; the other subcommands inspect, recover, pause,
resume, or cancel a goal.";
