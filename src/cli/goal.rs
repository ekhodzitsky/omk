use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
pub(crate) struct Args {
    #[command(subcommand)]
    pub(crate) command: GoalCommands,
}

#[derive(Subcommand, Debug)]
pub(crate) enum GoalCommands {
    /// Create a durable goal scaffold
    Run {
        /// High-level engineering goal
        goal: String,
        /// Keep working until ready once execution is implemented
        #[arg(long)]
        until_ready: bool,
        /// Time budget label, for example 8h or 7d
        #[arg(long)]
        budget_time: Option<String>,
        /// Maximum number of agents the future controller may use
        #[arg(long)]
        max_agents: Option<usize>,
    },
    /// List recorded goals
    List,
    /// Show compact status for a goal
    Status {
        /// Goal ID or "latest"
        #[arg(default_value = "latest")]
        goal_id: String,
    },
    /// Show goal state
    Show {
        /// Goal ID or "latest"
        #[arg(default_value = "latest")]
        goal_id: String,
        /// Output format
        #[arg(short, long, value_enum, default_value = "text")]
        format: OutputFormat,
        /// Output JSON (shortcut for --format json)
        #[arg(long)]
        json: bool,
    },
    /// Cancel a goal
    Cancel {
        /// Goal ID or "latest"
        #[arg(default_value = "latest")]
        goal_id: String,
    },
}

#[derive(Copy, Clone, Debug, clap::ValueEnum)]
pub(crate) enum OutputFormat {
    Text,
    Json,
    Md,
}

pub(crate) async fn run(args: Args) -> Result<()> {
    match args.command {
        GoalCommands::Run {
            goal,
            until_ready,
            budget_time,
            max_agents,
        } => {
            cmd_run(
                &goal,
                crate::runtime::goal::CreateGoalOptions {
                    until_ready,
                    budget_time,
                    max_agents,
                },
            )
            .await
        }
        GoalCommands::List => cmd_list().await,
        GoalCommands::Status { goal_id } => cmd_status(&goal_id).await,
        GoalCommands::Show {
            goal_id,
            format,
            json,
        } => cmd_show(&goal_id, format, json).await,
        GoalCommands::Cancel { goal_id } => cmd_cancel(&goal_id).await,
    }
}

async fn cmd_run(goal: &str, options: crate::runtime::goal::CreateGoalOptions) -> Result<()> {
    let state = crate::runtime::goal::create_goal(goal, options).await?;

    println!("Goal scaffold created: {}", state.goal_id);
    println!("Status: {}", state.status);
    println!("State: {}", state.state_dir.display());
    println!("Note: agent execution is not implemented in this scaffold yet.");
    println!("Next: omk goal show latest");
    Ok(())
}

async fn cmd_list() -> Result<()> {
    let goals = crate::runtime::goal::list_goals().await?;
    if goals.is_empty() {
        println!("No goals found.");
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
    println!("Status: {}", goal.status);
    println!("Goal: {}", goal.original_goal);
    println!("Updated: {}", goal.updated_at);
    Ok(())
}

async fn cmd_show(goal_id: &str, format: OutputFormat, json: bool) -> Result<()> {
    let goal = crate::runtime::goal::resolve_goal(goal_id).await?;
    let output_format = if json { OutputFormat::Json } else { format };

    match output_format {
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&goal)?),
        OutputFormat::Md => {
            println!("# Goal {}", goal.goal_id);
            println!();
            println!("- Status: `{}`", goal.status);
            println!("- Goal: {}", goal.original_goal);
            println!("- State: `{}`", goal.state_dir.display());
        }
        OutputFormat::Text => {
            println!("Goal {}", goal.goal_id);
            println!("Status: {}", goal.status);
            println!("Goal: {}", goal.original_goal);
            println!("Until ready: {}", goal.until_ready);
            if let Some(budget_time) = &goal.budget_time {
                println!("Budget time: {budget_time}");
            }
            if let Some(max_agents) = goal.max_agents {
                println!("Max agents: {max_agents}");
            }
            if let Some(failure) = &goal.failure {
                println!("Failure: {}", failure.reason);
            }
            println!("State: {}", goal.state_dir.display());
        }
    }

    Ok(())
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
