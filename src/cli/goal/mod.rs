//! `omk goal` CLI surface.
//!
//! User-facing wiring for the durable, proof-driven goal runtime. The actual
//! controller and proof logic live in `crate::runtime::goal`; this module owns
//! CLI parsing, validation of user input, and rendering output in `text`,
//! `json`, or `md` form.
//!
//! Layout:
//! - `help`     — long-form `--help` / `after_help` strings (prose).
//! - `validate` — eager input validation that runs before any side effects.
//! - `commands` — concrete `cmd_*` handlers and output rendering.
//!
//! All CLI strings are ASCII / locale-independent.

mod commands;
mod help;
mod types;
mod validate;

use anyhow::Result;
use clap::{Parser, Subcommand};

pub(crate) use types::{
    map_merge_policy, map_open_pr_policy, OpenPrFormat, OpenPrPolicy, OutputFormat,
};
use validate::{
    resolve_format, validate_budget_time, validate_decision_text, validate_goal_id,
    validate_goal_text, validate_optional_budget_tokens, validate_optional_budget_usd,
    validate_optional_max_agents,
};

#[derive(Parser, Debug)]
#[command(
    about = "Goal runtime (durable autonomous controller scaffold)",
    long_about = help::GOAL_LONG_ABOUT,
    after_help = help::GOAL_TOP_AFTER_HELP
)]
pub(crate) struct Args {
    #[command(subcommand)]
    pub(crate) command: GoalCommands,
}

#[derive(Subcommand, Debug)]
pub(crate) enum GoalCommands {
    /// Create a scaffold, or run the controller loop with --until-ready
    #[command(after_help = help::GOAL_RUN_AFTER_HELP)]
    Run {
        /// High-level engineering goal (quote it if it contains spaces)
        #[arg(value_name = "GOAL")]
        goal: String,
        /// Drive plan, verify, execute, and review until ready or blocked
        #[arg(long)]
        until_ready: bool,
        /// Wall-clock budget -- number with suffix s/m/h/d (for example: 8h, 7d)
        #[arg(long, value_name = "DURATION")]
        budget_time: Option<String>,
        /// Maximum estimated tokens the goal may spend (must be > 0)
        #[arg(long, value_name = "TOKENS")]
        budget_tokens: Option<u64>,
        /// Maximum estimated USD the goal may spend (must be > 0)
        #[arg(long, value_name = "USD")]
        budget_usd: Option<f64>,
        /// Maximum number of agents the controller may use (must be > 0)
        #[arg(long, value_name = "N")]
        max_agents: Option<usize>,
        /// Delivery policy: local, draft-pr, or auto-pr
        #[arg(long, value_enum, default_value = "local")]
        policy: types::OpenPrPolicy,
        /// Merge policy: disabled, manual, or gated
        #[arg(long, value_enum, default_value = "disabled")]
        merge_policy: types::MergePolicy,
        /// Run agents in per-slice git worktrees instead of the main repo
        #[arg(long)]
        slice_execution: bool,
        /// Enforce GitHub branch protection on main/master before integrator PR
        #[arg(long)]
        enforce_protection: bool,
    },
    /// Create a durable plan/proof scaffold without execution intent
    #[command(after_help = help::GOAL_PLAN_AFTER_HELP)]
    Plan {
        /// High-level engineering goal (quote it if it contains spaces)
        #[arg(value_name = "GOAL")]
        goal: String,
    },
    /// List recorded goals (newest first)
    #[command(after_help = help::GOAL_LIST_AFTER_HELP)]
    List,
    /// Show compact status for a goal
    #[command(after_help = help::GOAL_STATUS_AFTER_HELP)]
    Status {
        /// Goal ID or "latest"
        #[arg(default_value = "latest", value_name = "GOAL_ID")]
        goal_id: String,
    },
    /// Show full goal state
    #[command(after_help = help::GOAL_SHOW_AFTER_HELP)]
    Show {
        /// Goal ID or "latest"
        #[arg(default_value = "latest", value_name = "GOAL_ID")]
        goal_id: String,
        /// Output format
        #[arg(short, long, value_enum, default_value = "text")]
        format: types::OutputFormat,
        /// Shortcut for `--format json`
        #[arg(long, conflicts_with = "format")]
        json: bool,
    },
    /// Show the current goal proof artifact
    #[command(after_help = help::GOAL_PROOF_AFTER_HELP)]
    Proof {
        /// Goal ID or "latest"
        #[arg(default_value = "latest", value_name = "GOAL_ID")]
        goal_id: String,
        /// Output format
        #[arg(short, long, value_enum, default_value = "text")]
        format: types::OutputFormat,
        /// Shortcut for `--format json`
        #[arg(long, conflicts_with = "format")]
        json: bool,
    },
    /// Render a GitHub PR title/body from goal proof evidence
    #[command(after_help = help::GOAL_OPEN_PR_AFTER_HELP)]
    OpenPr {
        /// Goal ID or "latest"
        #[arg(default_value = "latest", value_name = "GOAL_ID")]
        goal_id: String,
        /// Render without network or GitHub creation
        #[arg(long)]
        dry_run: bool,
        /// Render metadata for a GitHub draft PR
        #[arg(long)]
        draft: bool,
        /// Delivery policy: local, draft-pr, or auto-pr
        #[arg(long, value_enum, default_value = "local")]
        policy: types::OpenPrPolicy,
        /// Base branch for the PR
        #[arg(long, value_name = "BRANCH")]
        base_branch: Option<String>,
        /// Output format
        #[arg(short, long, value_enum, default_value = "markdown")]
        format: types::OpenPrFormat,
    },
    /// Accept a proof-backed goal after explicit local integrator review
    #[command(after_help = help::GOAL_ACCEPT_AFTER_HELP)]
    Accept {
        /// Goal ID or "latest"
        #[arg(default_value = "latest", value_name = "GOAL_ID")]
        goal_id: String,
        /// Integrator acceptance summary
        #[arg(long, value_name = "TEXT")]
        summary: String,
    },
    /// Reject goal readiness after explicit local integrator review
    #[command(after_help = help::GOAL_REJECT_AFTER_HELP)]
    Reject {
        /// Goal ID or "latest"
        #[arg(default_value = "latest", value_name = "GOAL_ID")]
        goal_id: String,
        /// Rejection reason
        #[arg(long, value_name = "TEXT")]
        reason: String,
    },
    /// Replay the persisted goal timeline
    #[command(after_help = help::GOAL_REPLAY_AFTER_HELP)]
    Replay {
        /// Goal ID or "latest"
        #[arg(default_value = "latest", value_name = "GOAL_ID")]
        goal_id: String,
        /// Output format
        #[arg(short, long, value_enum, default_value = "text")]
        format: types::OutputFormat,
        /// Shortcut for `--format json`
        #[arg(long, conflicts_with = "format")]
        json: bool,
    },
    /// Show persisted budget checkpoints for a goal
    #[command(after_help = help::GOAL_BUDGET_AFTER_HELP)]
    Budget {
        /// Goal ID or "latest"
        #[arg(default_value = "latest", value_name = "GOAL_ID")]
        goal_id: String,
        /// Output format
        #[arg(short, long, value_enum, default_value = "text")]
        format: types::OutputFormat,
        /// Shortcut for `--format json`
        #[arg(long, conflicts_with = "format")]
        json: bool,
    },
    /// Extend an existing goal's budget (time, tokens, or USD)
    #[command(after_help = help::GOAL_BUDGET_ADD_AFTER_HELP)]
    BudgetAdd {
        /// Goal ID or "latest"
        #[arg(default_value = "latest", value_name = "GOAL_ID")]
        goal_id: String,
        /// Duration to add -- positive number with suffix s/m/h/d (for example: 1h, 30m)
        #[arg(long, value_name = "DURATION")]
        time: Option<String>,
        /// Estimated token budget to add (must be > 0)
        #[arg(long, value_name = "TOKENS")]
        tokens: Option<u64>,
        /// Estimated USD budget to add (must be > 0)
        #[arg(long, value_name = "USD")]
        usd: Option<f64>,
    },
    /// Run local verification gates and update the goal proof
    #[command(after_help = help::GOAL_VERIFY_AFTER_HELP)]
    Verify {
        /// Goal ID or "latest"
        #[arg(default_value = "latest", value_name = "GOAL_ID")]
        goal_id: String,
    },
    /// Run the current local controller execution step
    #[command(after_help = help::GOAL_EXECUTE_AFTER_HELP)]
    Execute {
        /// Goal ID or "latest"
        #[arg(default_value = "latest", value_name = "GOAL_ID")]
        goal_id: String,
    },
    /// Attach controller review and security-review evidence
    #[command(after_help = help::GOAL_REVIEW_AFTER_HELP)]
    Review {
        /// Goal ID or "latest"
        #[arg(default_value = "latest", value_name = "GOAL_ID")]
        goal_id: String,
    },
    /// Pause a goal until it is resumed
    #[command(after_help = help::GOAL_PAUSE_AFTER_HELP)]
    Pause {
        /// Goal ID or "latest"
        #[arg(default_value = "latest", value_name = "GOAL_ID")]
        goal_id: String,
    },
    /// Resume a paused or orphaned goal
    #[command(after_help = help::GOAL_RESUME_AFTER_HELP)]
    Resume {
        /// Goal ID or "latest"
        #[arg(default_value = "latest", value_name = "GOAL_ID")]
        goal_id: String,
        /// Automatically resume all orphaned goals (dead controller PID)
        #[arg(long)]
        auto: bool,
    },
    /// Cancel a goal and record a failure artifact
    #[command(after_help = help::GOAL_CANCEL_AFTER_HELP)]
    Cancel {
        /// Goal ID or "latest"
        #[arg(default_value = "latest", value_name = "GOAL_ID")]
        goal_id: String,
    },
    /// Merge the GitHub PR for a ready goal
    #[command(after_help = help::GOAL_MERGE_AFTER_HELP)]
    Merge {
        /// Goal ID or "latest"
        #[arg(default_value = "latest", value_name = "GOAL_ID")]
        goal_id: String,
    },
}

pub(crate) async fn run(args: Args) -> Result<()> {
    match args.command {
        GoalCommands::Run {
            goal,
            until_ready,
            budget_time,
            budget_tokens,
            budget_usd,
            max_agents,
            policy,
            merge_policy,
            slice_execution,
            enforce_protection,
        } => {
            let goal = validate_goal_text(&goal)?;
            let budget_time = validate_budget_time(budget_time.as_deref(), "--budget-time", false)?;
            validate_optional_budget_tokens(budget_tokens)?;
            validate_optional_budget_usd(budget_usd)?;
            validate_optional_max_agents(max_agents)?;
            commands::cmd_run(
                goal,
                crate::runtime::goal::CreateGoalOptions {
                    until_ready,
                    budget_time,
                    budget_tokens,
                    budget_usd,
                    max_agents,
                    delivery_policy: map_open_pr_policy(policy),
                    merge_policy: map_merge_policy(merge_policy),
                    slice_execution,
                    enforce_protection,
                },
            )
            .await
        }
        GoalCommands::Plan { goal } => {
            let goal = validate_goal_text(&goal)?;
            commands::cmd_plan(goal).await
        }
        GoalCommands::List => commands::cmd_list().await,
        GoalCommands::Status { goal_id } => {
            let goal_id = validate_goal_id(&goal_id)?;
            commands::cmd_status(goal_id).await
        }
        GoalCommands::Show {
            goal_id,
            format,
            json,
        } => {
            let goal_id = validate_goal_id(&goal_id)?;
            commands::cmd_show(goal_id, resolve_format(format, json)).await
        }
        GoalCommands::Proof {
            goal_id,
            format,
            json,
        } => {
            let goal_id = validate_goal_id(&goal_id)?;
            commands::cmd_proof(goal_id, resolve_format(format, json)).await
        }
        GoalCommands::OpenPr {
            goal_id,
            dry_run,
            draft,
            policy,
            base_branch,
            format,
        } => {
            let goal_id = validate_goal_id(&goal_id)?;
            commands::cmd_open_pr(goal_id, dry_run, draft, policy, base_branch, format).await
        }
        GoalCommands::Accept { goal_id, summary } => {
            let goal_id = validate_goal_id(&goal_id)?;
            let summary = validate_decision_text(&summary, "--summary")?;
            commands::cmd_accept(goal_id, summary).await
        }
        GoalCommands::Reject { goal_id, reason } => {
            let goal_id = validate_goal_id(&goal_id)?;
            let reason = validate_decision_text(&reason, "--reason")?;
            commands::cmd_reject(goal_id, reason).await
        }
        GoalCommands::Replay {
            goal_id,
            format,
            json,
        } => {
            let goal_id = validate_goal_id(&goal_id)?;
            commands::cmd_replay(goal_id, resolve_format(format, json)).await
        }
        GoalCommands::Budget {
            goal_id,
            format,
            json,
        } => {
            let goal_id = validate_goal_id(&goal_id)?;
            commands::cmd_budget(goal_id, resolve_format(format, json)).await
        }
        GoalCommands::BudgetAdd {
            goal_id,
            time,
            tokens,
            usd,
        } => {
            let goal_id = validate_goal_id(&goal_id)?;
            let time = validate_budget_time(time.as_deref(), "--time", true)?;
            validate_optional_budget_tokens(tokens)?;
            validate_optional_budget_usd(usd)?;
            if time.is_none() && tokens.is_none() && usd.is_none() {
                anyhow::bail!(
                    "Provide at least one budget extension: --time, --tokens, or --usd.\n\n\
                     Examples:\n  \
                     omk goal budget-add latest --time 1h\n  \
                     omk goal budget-add latest --tokens 500000\n  \
                     omk goal budget-add latest --usd 5"
                );
            }
            commands::cmd_budget_add(goal_id, time, tokens, usd).await
        }
        GoalCommands::Verify { goal_id } => {
            let goal_id = validate_goal_id(&goal_id)?;
            commands::cmd_verify(goal_id).await
        }
        GoalCommands::Execute { goal_id } => {
            let goal_id = validate_goal_id(&goal_id)?;
            commands::cmd_execute(goal_id).await
        }
        GoalCommands::Review { goal_id } => {
            let goal_id = validate_goal_id(&goal_id)?;
            commands::cmd_review(goal_id).await
        }
        GoalCommands::Pause { goal_id } => {
            let goal_id = validate_goal_id(&goal_id)?;
            commands::cmd_pause(goal_id).await
        }
        GoalCommands::Resume { goal_id, auto } => {
            if auto {
                commands::cmd_resume_auto().await
            } else {
                let goal_id = validate_goal_id(&goal_id)?;
                commands::cmd_resume(goal_id).await
            }
        }
        GoalCommands::Cancel { goal_id } => {
            let goal_id = validate_goal_id(&goal_id)?;
            commands::cmd_cancel(goal_id).await
        }
        GoalCommands::Merge { goal_id } => {
            let goal_id = validate_goal_id(&goal_id)?;
            commands::cmd_merge(goal_id).await
        }
    }
}
