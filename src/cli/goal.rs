//! `omk goal` CLI surface.
//!
//! User-facing wiring for the durable, proof-driven goal runtime. The actual
//! controller and proof logic live in `crate::runtime::goal`; this module is
//! responsible for CLI parsing, validation of user input, and rendering output
//! in `text`, `json`, or `md` form.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

const GOAL_TOP_AFTER_HELP: &str = "\
Examples:
  omk goal run \"Fix all clippy warnings\"
  omk goal run \"Rewrite project in Rust\" --until-ready --budget-time 8h
  omk goal list
  omk goal show latest --json
  omk goal verify latest
  omk goal execute latest
  omk goal review latest

Goal state is stored under the OMK state directory, one folder per goal:
  <state-dir>/goals/<goal-id>/
    goal.json          — durable state (status, phase, budgets)
    prd.md             — goal brief
    technical-plan.md  — controller plan
    task-graph.json    — task graph with retries/leases
    proof.json         — current proof bundle
    events.jsonl       — append-only event log
    decisions.jsonl    — controller decision log

Most commands accept either a concrete goal id or the alias `latest`.";

const GOAL_RUN_AFTER_HELP: &str = "\
Examples:
  omk goal run \"Fix all failing cargo tests\"
  omk goal run \"Ship CLI UX polish PR\" --until-ready
  omk goal run \"Migrate Python to Rust\" \\
      --until-ready --budget-time 7d --budget-tokens 2000000 --budget-usd 25 --max-agents 3

The command always creates a durable goal scaffold and exits. To execute the
goal, follow up with:
  omk goal execute latest
  omk goal review latest";

const GOAL_LIST_AFTER_HELP: &str = "\
Examples:
  omk goal list";

const GOAL_STATUS_AFTER_HELP: &str = "\
Examples:
  omk goal status
  omk goal status latest
  omk goal status goal-20260514-085416-149-ea263039";

const GOAL_SHOW_AFTER_HELP: &str = "\
Examples:
  omk goal show
  omk goal show latest --json
  omk goal show latest --format md > GOAL.md";

const GOAL_PROOF_AFTER_HELP: &str = "\
Examples:
  omk goal proof
  omk goal proof latest --json
  omk goal proof latest --format md";

const GOAL_REPLAY_AFTER_HELP: &str = "\
Examples:
  omk goal replay
  omk goal replay latest --json
  omk goal replay latest --format md";

const GOAL_BUDGET_AFTER_HELP: &str = "\
Examples:
  omk goal budget
  omk goal budget latest --json";

const GOAL_BUDGET_ADD_AFTER_HELP: &str = "\
Examples:
  omk goal budget-add --time 1h
  omk goal budget-add latest --tokens 500000
  omk goal budget-add latest --time 30m --usd 5

At least one of --time / --tokens / --usd must be provided.";

const GOAL_VERIFY_AFTER_HELP: &str = "\
Examples:
  omk goal verify
  omk goal verify latest

Runs the configured local verification gates (cargo fmt, check, clippy, test,
doc by default) and writes the result into the goal proof.";

const GOAL_EXECUTE_AFTER_HELP: &str = "\
Examples:
  omk goal execute
  omk goal execute latest";

const GOAL_REVIEW_AFTER_HELP: &str = "\
Examples:
  omk goal review
  omk goal review latest";

const GOAL_PAUSE_AFTER_HELP: &str = "\
Examples:
  omk goal pause
  omk goal pause latest";

const GOAL_RESUME_AFTER_HELP: &str = "\
Examples:
  omk goal resume
  omk goal resume latest";

const GOAL_CANCEL_AFTER_HELP: &str = "\
Examples:
  omk goal cancel
  omk goal cancel latest

Records a `failure.json` artifact and stops further execution.";

const GOAL_PLAN_AFTER_HELP: &str = "\
Examples:
  omk goal plan \"Investigate flaky verifier tests\"";

#[derive(Parser, Debug)]
#[command(
    about = "Goal runtime (durable autonomous controller scaffold)",
    long_about = "Goal runtime — durable, proof-driven engineering goals.\n\n\
                  Each goal owns a state directory with PRD, technical plan, task graph,\n\
                  event log, and proof bundle. Use the subcommands below to create,\n\
                  inspect, execute, verify, review, pause, resume, or cancel a goal.",
    after_help = GOAL_TOP_AFTER_HELP
)]
pub(crate) struct Args {
    #[command(subcommand)]
    pub(crate) command: GoalCommands,
}

#[derive(Subcommand, Debug)]
pub(crate) enum GoalCommands {
    /// Create a durable goal scaffold
    #[command(after_help = GOAL_RUN_AFTER_HELP)]
    Run {
        /// High-level engineering goal (quote it if it contains spaces)
        #[arg(value_name = "GOAL")]
        goal: String,
        /// Keep working until the goal is proof-backed ready (controller hint)
        #[arg(long)]
        until_ready: bool,
        /// Wall-clock budget — positive number with suffix s/m/h/d (for example: 8h, 7d)
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
    #[command(after_help = GOAL_PLAN_AFTER_HELP)]
    Plan {
        /// High-level engineering goal (quote it if it contains spaces)
        #[arg(value_name = "GOAL")]
        goal: String,
    },
    /// List recorded goals (newest first)
    #[command(after_help = GOAL_LIST_AFTER_HELP)]
    List,
    /// Show compact status for a goal
    #[command(after_help = GOAL_STATUS_AFTER_HELP)]
    Status {
        /// Goal ID or "latest"
        #[arg(default_value = "latest", value_name = "GOAL_ID")]
        goal_id: String,
    },
    /// Show full goal state
    #[command(after_help = GOAL_SHOW_AFTER_HELP)]
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
    #[command(after_help = GOAL_PROOF_AFTER_HELP)]
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
    /// Replay the persisted goal timeline
    #[command(after_help = GOAL_REPLAY_AFTER_HELP)]
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
    #[command(after_help = GOAL_BUDGET_AFTER_HELP)]
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
    #[command(after_help = GOAL_BUDGET_ADD_AFTER_HELP)]
    BudgetAdd {
        /// Goal ID or "latest"
        #[arg(default_value = "latest", value_name = "GOAL_ID")]
        goal_id: String,
        /// Duration to add — positive number with suffix s/m/h/d (for example: 1h, 30m)
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
    #[command(after_help = GOAL_VERIFY_AFTER_HELP)]
    Verify {
        /// Goal ID or "latest"
        #[arg(default_value = "latest", value_name = "GOAL_ID")]
        goal_id: String,
    },
    /// Run the current local controller execution step
    #[command(after_help = GOAL_EXECUTE_AFTER_HELP)]
    Execute {
        /// Goal ID or "latest"
        #[arg(default_value = "latest", value_name = "GOAL_ID")]
        goal_id: String,
    },
    /// Attach controller review and security-review evidence
    #[command(after_help = GOAL_REVIEW_AFTER_HELP)]
    Review {
        /// Goal ID or "latest"
        #[arg(default_value = "latest", value_name = "GOAL_ID")]
        goal_id: String,
    },
    /// Pause a goal until it is resumed
    #[command(after_help = GOAL_PAUSE_AFTER_HELP)]
    Pause {
        /// Goal ID or "latest"
        #[arg(default_value = "latest", value_name = "GOAL_ID")]
        goal_id: String,
    },
    /// Resume a paused goal
    #[command(after_help = GOAL_RESUME_AFTER_HELP)]
    Resume {
        /// Goal ID or "latest"
        #[arg(default_value = "latest", value_name = "GOAL_ID")]
        goal_id: String,
    },
    /// Cancel a goal and record a failure artifact
    #[command(after_help = GOAL_CANCEL_AFTER_HELP)]
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
            let budget_time = validate_optional_budget_time(budget_time.as_deref())?;
            validate_optional_budget_tokens(budget_tokens)?;
            validate_optional_budget_usd(budget_usd)?;
            validate_optional_max_agents(max_agents)?;
            cmd_run(
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
            cmd_plan(goal).await
        }
        GoalCommands::List => cmd_list().await,
        GoalCommands::Status { goal_id } => {
            let goal_id = validate_goal_id(&goal_id)?;
            cmd_status(goal_id).await
        }
        GoalCommands::Show {
            goal_id,
            format,
            json,
        } => {
            let goal_id = validate_goal_id(&goal_id)?;
            cmd_show(goal_id, resolve_format(format, json)).await
        }
        GoalCommands::Proof {
            goal_id,
            format,
            json,
        } => {
            let goal_id = validate_goal_id(&goal_id)?;
            cmd_proof(goal_id, resolve_format(format, json)).await
        }
        GoalCommands::Replay {
            goal_id,
            format,
            json,
        } => {
            let goal_id = validate_goal_id(&goal_id)?;
            cmd_replay(goal_id, resolve_format(format, json)).await
        }
        GoalCommands::Budget {
            goal_id,
            format,
            json,
        } => {
            let goal_id = validate_goal_id(&goal_id)?;
            cmd_budget(goal_id, resolve_format(format, json)).await
        }
        GoalCommands::BudgetAdd {
            goal_id,
            time,
            tokens,
            usd,
        } => {
            let goal_id = validate_goal_id(&goal_id)?;
            let time = validate_positive_budget_time(time.as_deref(), "--time")?;
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
            cmd_budget_add(goal_id, time, tokens, usd).await
        }
        GoalCommands::Verify { goal_id } => {
            let goal_id = validate_goal_id(&goal_id)?;
            cmd_verify(goal_id).await
        }
        GoalCommands::Execute { goal_id } => {
            let goal_id = validate_goal_id(&goal_id)?;
            cmd_execute(goal_id).await
        }
        GoalCommands::Review { goal_id } => {
            let goal_id = validate_goal_id(&goal_id)?;
            cmd_review(goal_id).await
        }
        GoalCommands::Pause { goal_id } => {
            let goal_id = validate_goal_id(&goal_id)?;
            cmd_pause(goal_id).await
        }
        GoalCommands::Resume { goal_id } => {
            let goal_id = validate_goal_id(&goal_id)?;
            cmd_resume(goal_id).await
        }
        GoalCommands::Cancel { goal_id } => {
            let goal_id = validate_goal_id(&goal_id)?;
            cmd_cancel(goal_id).await
        }
    }
}

fn resolve_format(format: OutputFormat, json: bool) -> OutputFormat {
    if json {
        OutputFormat::Json
    } else {
        format
    }
}

fn validate_goal_text(goal: &str) -> Result<&str> {
    let trimmed = goal.trim();
    if trimmed.is_empty() {
        anyhow::bail!(
            "goal text cannot be empty.\n\nExample:\n  omk goal run \"Fix all clippy warnings\""
        );
    }
    Ok(trimmed)
}

fn validate_goal_id(goal_id: &str) -> Result<&str> {
    let trimmed = goal_id.trim();
    if trimmed.is_empty() {
        anyhow::bail!(
            "goal id cannot be empty.\n\nUse `latest` or a concrete goal id (see `omk goal list`)."
        );
    }
    Ok(trimmed)
}

fn validate_optional_budget_time(value: Option<&str>) -> Result<Option<String>> {
    match value {
        Some(value) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                anyhow::bail!("--budget-time cannot be empty.\n\nExample: --budget-time 8h");
            }
            crate::runtime::goal::parse_budget_duration(trimmed)
                .context("invalid --budget-time")?;
            Ok(Some(trimmed.to_string()))
        }
        None => Ok(None),
    }
}

fn validate_positive_budget_time(value: Option<&str>, flag: &str) -> Result<Option<String>> {
    match value {
        Some(value) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                anyhow::bail!("{flag} cannot be empty.\n\nExample: {flag} 1h");
            }
            let secs = crate::runtime::goal::parse_budget_duration(trimmed)
                .with_context(|| format!("invalid {flag}"))?;
            if secs == 0 {
                anyhow::bail!("{flag} must be greater than zero (for example: {flag} 1h).");
            }
            Ok(Some(trimmed.to_string()))
        }
        None => Ok(None),
    }
}

fn validate_optional_budget_tokens(value: Option<u64>) -> Result<()> {
    if let Some(value) = value {
        if value == 0 {
            anyhow::bail!("--budget-tokens must be greater than zero.");
        }
    }
    Ok(())
}

fn validate_optional_budget_usd(value: Option<f64>) -> Result<()> {
    if let Some(value) = value {
        if !value.is_finite() || value <= 0.0 {
            anyhow::bail!(
                "--budget-usd must be a positive, finite number (for example: --budget-usd 5)."
            );
        }
    }
    Ok(())
}

fn validate_optional_max_agents(value: Option<usize>) -> Result<()> {
    if let Some(value) = value {
        if value == 0 {
            anyhow::bail!("--max-agents must be greater than zero.");
        }
    }
    Ok(())
}

async fn cmd_run(goal: &str, options: crate::runtime::goal::CreateGoalOptions) -> Result<()> {
    let state = crate::runtime::goal::create_goal(goal, options).await?;

    println!("Goal scaffold created: {}", state.goal_id);
    println!("  Status: {}", state.status);
    println!("  Phase:  {}", state.phase);
    println!("  State:  {}", state.state_dir.display());
    println!(
        "  Proof:  {}",
        state
            .state_dir
            .join(crate::runtime::goal::GOAL_PROOF_FILE)
            .display()
    );
    if state.status == crate::runtime::goal::GoalStatus::BlockedOnHuman {
        if let Some(failure) = &state.failure {
            println!();
            println!("Decision needed: {}", failure.reason);
        }
        println!();
        println!("Next: refine the goal with testable success criteria, then run it again.");
        println!("  Example:");
        println!("    omk goal run \"Fix all failing cargo tests in src/runtime/goal\"");
    } else {
        println!();
        println!("Next steps:");
        println!("  1. Inspect the scaffold:  omk goal show latest");
        println!("  2. Run verification:      omk goal verify latest");
        println!("  3. Execute agent wave:    omk goal execute latest");
        println!("  4. Attach reviews:        omk goal review latest");
    }
    Ok(())
}

async fn cmd_plan(goal: &str) -> Result<()> {
    let state = crate::runtime::goal::plan_goal(goal).await?;

    println!("Goal plan created: {}", state.goal_id);
    println!("  Status: {}", state.status);
    println!("  Phase:  {}", state.phase);
    println!("  State:  {}", state.state_dir.display());
    println!(
        "  Proof:  {}",
        state
            .state_dir
            .join(crate::runtime::goal::GOAL_PROOF_FILE)
            .display()
    );
    println!();
    println!("Next steps:");
    println!("  1. Inspect the plan:  omk goal show latest");
    println!("  2. Promote to run:    omk goal run \"<refined goal>\"");
    Ok(())
}

async fn cmd_list() -> Result<()> {
    let goals = crate::runtime::goal::list_goals().await?;
    if goals.is_empty() {
        println!("No goals found.");
        println!();
        println!("Create one with:");
        println!("  omk goal run \"<engineering goal>\"");
        return Ok(());
    }

    println!("Goals ({}):", goals.len());
    for goal in goals {
        println!(
            "  [{:16}] {}  {}",
            goal.status, goal.goal_id, goal.original_goal
        );
    }
    Ok(())
}

async fn cmd_status(goal_id: &str) -> Result<()> {
    let goal = crate::runtime::goal::resolve_goal(goal_id).await?;
    println!("Goal status — {}", goal.goal_id);
    println!("  Status:  {}", goal.status);
    println!("  Phase:   {}", goal.phase);
    println!("  Goal:    {}", goal.original_goal);
    println!("  Updated: {}", goal.updated_at);
    Ok(())
}

async fn cmd_show(goal_id: &str, format: OutputFormat) -> Result<()> {
    let goal = crate::runtime::goal::resolve_goal(goal_id).await?;

    match format {
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&goal)?),
        OutputFormat::Md => {
            println!("# Goal {}", goal.goal_id);
            println!();
            println!("- Status: `{}`", goal.status);
            println!("- Phase: `{}`", goal.phase);
            println!("- Goal: {}", goal.original_goal);
            println!("- State: `{}`", goal.state_dir.display());
            println!(
                "- Proof: `{}`",
                goal.state_dir
                    .join(crate::runtime::goal::GOAL_PROOF_FILE)
                    .display()
            );
            println!();
            println!("## Artifacts");
            for artifact in &goal.artifacts {
                println!("- `{}`: `{}`", artifact.kind, artifact.path.display());
            }
        }
        OutputFormat::Text => {
            println!("Goal {}", goal.goal_id);
            println!("Status: {}", goal.status);
            println!("Phase: {}", goal.phase);
            println!("Goal: {}", goal.original_goal);
            println!("Until ready: {}", goal.until_ready);
            if let Some(budget_time) = &goal.budget_time {
                println!("Budget time: {budget_time}");
            }
            if let Some(budget_tokens) = goal.budget_tokens {
                println!("Budget tokens: {budget_tokens}");
            }
            if let Some(budget_usd) = goal.budget_usd {
                println!("Budget USD: {budget_usd:.6}");
            }
            if let Some(max_agents) = goal.max_agents {
                println!("Max agents: {max_agents}");
            }
            if let Some(failure) = &goal.failure {
                println!("Failure: {}", failure.reason);
            }
            if !goal.artifacts.is_empty() {
                println!("Artifacts:");
                for artifact in &goal.artifacts {
                    println!("  {}: {}", artifact.kind, artifact.path.display());
                }
            }
            println!(
                "Proof: {}",
                goal.state_dir
                    .join(crate::runtime::goal::GOAL_PROOF_FILE)
                    .display()
            );
            println!("State: {}", goal.state_dir.display());
        }
    }

    Ok(())
}

async fn cmd_proof(goal_id: &str, format: OutputFormat) -> Result<()> {
    let proof = crate::runtime::goal::resolve_goal_proof(goal_id).await?;

    match format {
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&proof)?),
        OutputFormat::Md => {
            println!("# Goal Proof {}", proof.goal_id);
            println!();
            println!("- Status: `{}`", proof.status);
            println!("- Readiness: {}", proof.readiness);
            println!("- Tasks: {}", proof.task_graph_summary.total_tasks);
            if !proof.known_gaps.is_empty() {
                println!();
                println!("## Known Gaps");
                for gap in &proof.known_gaps {
                    println!("- {gap}");
                }
            }
        }
        OutputFormat::Text => {
            println!("Goal proof {}", proof.goal_id);
            println!("Status: {}", proof.status);
            println!("Readiness: {}", proof.readiness);
            println!("Tasks: {}", proof.task_graph_summary.total_tasks);
            if !proof.known_gaps.is_empty() {
                println!("Known gaps:");
                for gap in &proof.known_gaps {
                    println!("  - {gap}");
                }
            }
        }
    }

    Ok(())
}

async fn cmd_budget(goal_id: &str, format: OutputFormat) -> Result<()> {
    let report = crate::runtime::goal::goal_budget(goal_id).await?;

    match format {
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&report)?),
        OutputFormat::Md => {
            println!("# Goal Budget {}", report.goal_id);
            println!();
            print_budget_summary_md(&report);
            println!();
            println!("## Checkpoints");
            for checkpoint in &report.checkpoints {
                println!(
                    "- `{}` `{}` status=`{}` phase=`{}` remaining={}",
                    checkpoint.recorded_at,
                    checkpoint.label,
                    checkpoint.status,
                    checkpoint.phase,
                    format_optional_secs(checkpoint.remaining_budget_secs)
                );
            }
        }
        OutputFormat::Text => {
            println!("Goal budget {}", report.goal_id);
            print_budget_summary_text(&report);
            println!("Checkpoints: {}", report.checkpoints.len());
            for checkpoint in &report.checkpoints {
                println!(
                    "  {}  {:18} status={} phase={} remaining={}",
                    checkpoint.recorded_at,
                    checkpoint.label,
                    checkpoint.status,
                    checkpoint.phase,
                    format_optional_secs(checkpoint.remaining_budget_secs)
                );
            }
        }
    }

    Ok(())
}

async fn cmd_budget_add(
    goal_id: &str,
    time: Option<String>,
    tokens: Option<u64>,
    usd: Option<f64>,
) -> Result<()> {
    let goal = crate::runtime::goal::add_goal_budget_limits(
        goal_id,
        crate::runtime::goal::GoalBudgetAdd {
            time: time.clone(),
            tokens,
            usd,
        },
    )
    .await?;
    println!("Budget added: {}", goal.goal_id);
    println!("Status: {}", goal.status);
    println!(
        "Budget time: {}",
        goal.budget_time.as_deref().unwrap_or("unbounded")
    );
    println!(
        "Budget tokens: {}",
        goal.budget_tokens
            .map(|value| value.to_string())
            .unwrap_or_else(|| "unbounded".to_string())
    );
    println!(
        "Budget USD: {}",
        goal.budget_usd
            .map(|value| format!("{value:.6}"))
            .unwrap_or_else(|| "unbounded".to_string())
    );
    if let Some(time) = time {
        println!("Time added: {time}");
    }
    if let Some(tokens) = tokens {
        println!("Tokens added: {tokens}");
    }
    if let Some(usd) = usd {
        println!("USD added: {usd:.6}");
    }
    Ok(())
}

fn print_budget_summary_text(report: &crate::runtime::goal::GoalBudgetReport) {
    println!(
        "Budget time: {}",
        report.budget_time.as_deref().unwrap_or("unbounded")
    );
    println!("Total: {}", format_optional_secs(report.total_budget_secs));
    println!(
        "Tokens: used={} budget={} remaining={}",
        report.used_tokens,
        report
            .budget_tokens
            .map(|value| value.to_string())
            .unwrap_or_else(|| "unbounded".to_string()),
        report
            .remaining_budget_tokens
            .map(|value| value.to_string())
            .unwrap_or_else(|| "unbounded".to_string())
    );
    println!(
        "Cost: estimated=${:.6} budget={} remaining={}",
        report.estimated_cost_usd,
        report
            .budget_usd
            .map(|value| format!("${value:.6}"))
            .unwrap_or_else(|| "unbounded".to_string()),
        report
            .remaining_budget_usd
            .map(|value| format!("${value:.6}"))
            .unwrap_or_else(|| "unbounded".to_string())
    );
    if let Some(latest) = &report.latest {
        println!(
            "Elapsed: {}",
            format_optional_secs(Some(latest.elapsed_since_created_secs))
        );
        println!(
            "Remaining: {}",
            format_optional_secs(latest.remaining_budget_secs)
        );
    } else {
        println!("Elapsed: unknown");
        println!("Remaining: unknown");
    }
}

fn print_budget_summary_md(report: &crate::runtime::goal::GoalBudgetReport) {
    println!(
        "- Budget time: `{}`",
        report.budget_time.as_deref().unwrap_or("unbounded")
    );
    println!(
        "- Total: `{}`",
        format_optional_secs(report.total_budget_secs)
    );
    println!(
        "- Tokens: used=`{}` budget=`{}` remaining=`{}`",
        report.used_tokens,
        report
            .budget_tokens
            .map(|value| value.to_string())
            .unwrap_or_else(|| "unbounded".to_string()),
        report
            .remaining_budget_tokens
            .map(|value| value.to_string())
            .unwrap_or_else(|| "unbounded".to_string())
    );
    println!(
        "- Cost: estimated=`${:.6}` budget=`{}` remaining=`{}`",
        report.estimated_cost_usd,
        report
            .budget_usd
            .map(|value| format!("${value:.6}"))
            .unwrap_or_else(|| "unbounded".to_string()),
        report
            .remaining_budget_usd
            .map(|value| format!("${value:.6}"))
            .unwrap_or_else(|| "unbounded".to_string())
    );
    if let Some(latest) = &report.latest {
        println!(
            "- Elapsed: `{}`",
            format_optional_secs(Some(latest.elapsed_since_created_secs))
        );
        println!(
            "- Remaining: `{}`",
            format_optional_secs(latest.remaining_budget_secs)
        );
    } else {
        println!("- Elapsed: `unknown`");
        println!("- Remaining: `unknown`");
    }
}

fn format_optional_secs(value: Option<u64>) -> String {
    value
        .map(|value| format!("{value}s"))
        .unwrap_or_else(|| "unbounded".to_string())
}

async fn cmd_replay(goal_id: &str, format: OutputFormat) -> Result<()> {
    let replay = crate::runtime::goal::replay_goal(goal_id).await?;

    match format {
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&replay)?),
        OutputFormat::Md => {
            println!("# Goal Replay {}", replay.goal_id);
            println!();
            println!("- Status: `{}`", replay.status);
            println!("- Phase: `{}`", replay.phase);
            println!("- Events: {}", replay.event_count);
            println!(
                "- Tasks: {}/{} done",
                replay.task_graph_summary.done_tasks, replay.task_graph_summary.total_tasks
            );
            println!();
            println!("## Timeline");
            for entry in &replay.timeline {
                if let Some(summary) = &entry.summary {
                    println!("- `{}` `{}` {}", entry.ts, entry.kind, summary);
                } else {
                    println!("- `{}` `{}`", entry.ts, entry.kind);
                }
            }
        }
        OutputFormat::Text => {
            println!("Goal replay {}", replay.goal_id);
            println!("Status: {}", replay.status);
            println!("Phase: {}", replay.phase);
            println!("Events: {}", replay.event_count);
            println!(
                "Tasks: {}/{} done",
                replay.task_graph_summary.done_tasks, replay.task_graph_summary.total_tasks
            );
            println!("Timeline:");
            for entry in &replay.timeline {
                if let Some(summary) = &entry.summary {
                    println!("  {}  {:22} {}", entry.ts, entry.kind, summary);
                } else {
                    println!("  {}  {}", entry.ts, entry.kind);
                }
            }
        }
    }

    Ok(())
}

async fn cmd_verify(goal_id: &str) -> Result<()> {
    let project_dir = project_dir_for_goal()?;
    let proof = crate::runtime::goal::verify_goal(goal_id, &project_dir).await?;

    println!("Verification: {}", proof.status);
    println!("Readiness: {}", proof.readiness);
    if proof.gates.is_empty() {
        println!("Gates: none");
    } else {
        println!("Gates:");
        for gate in &proof.gates {
            let status = if gate.passed { "passed" } else { "failed" };
            println!("  {}: {}", gate.name, status);
        }
    }
    println!("Proof: {}", crate::runtime::goal::GOAL_PROOF_FILE);
    Ok(())
}

async fn cmd_execute(goal_id: &str) -> Result<()> {
    let project_dir = project_dir_for_goal()?;
    let proof = crate::runtime::goal::execute_goal(goal_id, &project_dir).await?;
    let goal = crate::runtime::goal::resolve_goal(goal_id).await?;
    let task_graph = crate::runtime::goal::GoalTaskGraph::load(&goal.state_dir).await?;

    println!("Execution: {}", proof.status);
    println!("Readiness: {}", proof.readiness);
    println!(
        "Done tasks: {}/{}",
        proof.task_graph_summary.done_tasks, proof.task_graph_summary.total_tasks
    );
    print_task_status(&task_graph, "goal-local-verify");
    print_task_status(&task_graph, "goal-agent-execute");
    print_task_status(&task_graph, "goal-review");
    print_task_status(&task_graph, "goal-security-review");
    println!("Proof: {}", crate::runtime::goal::GOAL_PROOF_FILE);
    Ok(())
}

async fn cmd_review(goal_id: &str) -> Result<()> {
    let project_dir = project_dir_for_goal()?;
    let proof = crate::runtime::goal::review_goal(goal_id, &project_dir).await?;
    let goal = crate::runtime::goal::resolve_goal(goal_id).await?;
    let task_graph = crate::runtime::goal::GoalTaskGraph::load(&goal.state_dir).await?;

    println!("Review: {}", proof.status);
    println!("Readiness: {}", proof.readiness);
    println!(
        "Done tasks: {}/{}",
        proof.task_graph_summary.done_tasks, proof.task_graph_summary.total_tasks
    );
    print_task_status(&task_graph, "goal-review");
    print_task_status(&task_graph, "goal-security-review");
    println!("Proof: {}", crate::runtime::goal::GOAL_PROOF_FILE);
    Ok(())
}

fn project_dir_for_goal() -> Result<std::path::PathBuf> {
    std::env::current_dir().with_context(|| {
        "failed to read current working directory.\n\
         Run this command from the project root you want to verify, or `cd` into a readable directory."
            .to_string()
    })
}

fn print_task_status(task_graph: &crate::runtime::goal::GoalTaskGraph, task_id: &str) {
    if let Some(task) = task_graph.tasks.iter().find(|task| task.id == task_id) {
        println!("{}: {}", task.id, task.status);
    }
}

async fn cmd_cancel(goal_id: &str) -> Result<()> {
    let goal = crate::runtime::goal::cancel_goal(goal_id).await?;
    println!("Goal {} cancelled", goal.goal_id);
    println!("Status: {}", goal.status);
    println!(
        "Failure artifact: {}",
        goal.state_dir
            .join(crate::runtime::goal::GOAL_FAILURE_FILE)
            .display()
    );
    Ok(())
}

async fn cmd_pause(goal_id: &str) -> Result<()> {
    let goal = crate::runtime::goal::pause_goal(goal_id).await?;
    println!("Goal {} paused", goal.goal_id);
    println!("Status: {}", goal.status);
    println!("Phase: {}", goal.phase);
    println!("Updated: {}", goal.updated_at);
    println!();
    println!("Resume with: omk goal resume {}", goal.goal_id);
    Ok(())
}

async fn cmd_resume(goal_id: &str) -> Result<()> {
    let goal = crate::runtime::goal::resume_goal(goal_id).await?;
    println!("Goal {} resumed", goal.goal_id);
    println!("Status: {}", goal.status);
    println!("Phase: {}", goal.phase);
    println!("Updated: {}", goal.updated_at);
    Ok(())
}
