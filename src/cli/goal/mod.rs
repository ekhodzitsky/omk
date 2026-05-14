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
mod validate;

use anyhow::Result;
use clap::{Parser, Subcommand};

use validate::{
    resolve_format, validate_budget_time, validate_goal_id, validate_goal_text,
    validate_optional_budget_tokens, validate_optional_budget_usd, validate_optional_max_agents,
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
    /// Create a durable goal scaffold
    #[command(after_help = help::GOAL_RUN_AFTER_HELP)]
    Run {
        /// High-level engineering goal (quote it if it contains spaces)
        #[arg(value_name = "GOAL")]
        goal: String,
        /// Keep working until the goal is proof-backed ready (controller hint)
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
        format: OutputFormat,
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
        format: OutputFormat,
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
        /// Output format
        #[arg(short, long, value_enum, default_value = "markdown")]
        format: OpenPrFormat,
    },
    /// Replay the persisted goal timeline
    #[command(after_help = help::GOAL_REPLAY_AFTER_HELP)]
    Replay {
        /// Goal ID or "latest"
        #[arg(default_value = "latest", value_name = "GOAL_ID")]
        goal_id: String,
        /// Output format
        #[arg(short, long, value_enum, default_value = "text")]
        format: OutputFormat,
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
        format: OutputFormat,
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
    /// Resume a paused goal
    #[command(after_help = help::GOAL_RESUME_AFTER_HELP)]
    Resume {
        /// Goal ID or "latest"
        #[arg(default_value = "latest", value_name = "GOAL_ID")]
        goal_id: String,
    },
    /// Cancel a goal and record a failure artifact
    #[command(after_help = help::GOAL_CANCEL_AFTER_HELP)]
    Cancel {
        /// Goal ID or "latest"
        #[arg(default_value = "latest", value_name = "GOAL_ID")]
        goal_id: String,
    },
}

#[derive(Copy, Clone, Debug, clap::ValueEnum)]
pub(crate) enum OutputFormat {
    /// Human-readable text (default)
    Text,
    /// Machine-readable JSON
    Json,
    /// Markdown for documentation pipelines
    Md,
}

#[derive(Copy, Clone, Debug, clap::ValueEnum)]
pub(crate) enum OpenPrFormat {
    /// Human-readable text
    Text,
    /// Machine-readable JSON
    Json,
    /// Markdown PR title/body draft
    #[value(alias = "md")]
    Markdown,
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
            format,
        } => {
            let goal_id = validate_goal_id(&goal_id)?;
            commands::cmd_open_pr(goal_id, dry_run, format).await
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
        GoalCommands::Resume { goal_id } => {
            let goal_id = validate_goal_id(&goal_id)?;
            commands::cmd_resume(goal_id).await
        }
        GoalCommands::Cancel { goal_id } => {
            let goal_id = validate_goal_id(&goal_id)?;
            commands::cmd_cancel(goal_id).await
        }
    }
}
