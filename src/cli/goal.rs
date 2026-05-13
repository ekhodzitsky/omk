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
    /// Create a durable plan/proof scaffold without future execution intent
    Plan {
        /// High-level engineering goal
        goal: String,
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
    /// Show the goal proof artifact
    Proof {
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
    /// Replay the persisted goal timeline
    Replay {
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
    /// Show persisted budget checkpoints for a goal
    Budget {
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
    /// Run local verification gates and update the goal proof
    Verify {
        /// Goal ID or "latest"
        #[arg(default_value = "latest")]
        goal_id: String,
    },
    /// Run the current local controller execution step
    Execute {
        /// Goal ID or "latest"
        #[arg(default_value = "latest")]
        goal_id: String,
    },
    /// Review goal execution and security evidence
    Review {
        /// Goal ID or "latest"
        #[arg(default_value = "latest")]
        goal_id: String,
    },
    /// Pause a goal until it is resumed
    Pause {
        /// Goal ID or "latest"
        #[arg(default_value = "latest")]
        goal_id: String,
    },
    /// Resume a paused goal
    Resume {
        /// Goal ID or "latest"
        #[arg(default_value = "latest")]
        goal_id: String,
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
        GoalCommands::Plan { goal } => cmd_plan(&goal).await,
        GoalCommands::List => cmd_list().await,
        GoalCommands::Status { goal_id } => cmd_status(&goal_id).await,
        GoalCommands::Show {
            goal_id,
            format,
            json,
        } => cmd_show(&goal_id, format, json).await,
        GoalCommands::Proof {
            goal_id,
            format,
            json,
        } => cmd_proof(&goal_id, format, json).await,
        GoalCommands::Replay {
            goal_id,
            format,
            json,
        } => cmd_replay(&goal_id, format, json).await,
        GoalCommands::Budget {
            goal_id,
            format,
            json,
        } => cmd_budget(&goal_id, format, json).await,
        GoalCommands::Verify { goal_id } => cmd_verify(&goal_id).await,
        GoalCommands::Execute { goal_id } => cmd_execute(&goal_id).await,
        GoalCommands::Review { goal_id } => cmd_review(&goal_id).await,
        GoalCommands::Pause { goal_id } => cmd_pause(&goal_id).await,
        GoalCommands::Resume { goal_id } => cmd_resume(&goal_id).await,
        GoalCommands::Cancel { goal_id } => cmd_cancel(&goal_id).await,
    }
}

async fn cmd_run(goal: &str, options: crate::runtime::goal::CreateGoalOptions) -> Result<()> {
    let state = crate::runtime::goal::create_goal(goal, options).await?;

    println!("Goal scaffold created: {}", state.goal_id);
    println!("Status: {}", state.status);
    println!("Phase: {}", state.phase);
    println!("State: {}", state.state_dir.display());
    println!(
        "Proof: {}",
        state
            .state_dir
            .join(crate::runtime::goal::GOAL_PROOF_FILE)
            .display()
    );
    println!("Note: run `omk goal execute latest`, then `omk goal review latest` for evidence.");
    println!("Next: omk goal show latest");
    Ok(())
}

async fn cmd_plan(goal: &str) -> Result<()> {
    let state = crate::runtime::goal::plan_goal(goal).await?;

    println!("Goal plan created: {}", state.goal_id);
    println!("Status: {}", state.status);
    println!("Phase: {}", state.phase);
    println!("State: {}", state.state_dir.display());
    println!(
        "Proof: {}",
        state
            .state_dir
            .join(crate::runtime::goal::GOAL_PROOF_FILE)
            .display()
    );
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
    println!("Phase: {}", goal.phase);
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

async fn cmd_proof(goal_id: &str, format: OutputFormat, json: bool) -> Result<()> {
    let proof = crate::runtime::goal::resolve_goal_proof(goal_id).await?;
    let output_format = if json { OutputFormat::Json } else { format };

    match output_format {
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

async fn cmd_budget(goal_id: &str, format: OutputFormat, json: bool) -> Result<()> {
    let report = crate::runtime::goal::goal_budget(goal_id).await?;
    let output_format = if json { OutputFormat::Json } else { format };

    match output_format {
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

fn print_budget_summary_text(report: &crate::runtime::goal::GoalBudgetReport) {
    println!(
        "Budget time: {}",
        report.budget_time.as_deref().unwrap_or("unbounded")
    );
    println!("Total: {}", format_optional_secs(report.total_budget_secs));
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

async fn cmd_replay(goal_id: &str, format: OutputFormat, json: bool) -> Result<()> {
    let replay = crate::runtime::goal::replay_goal(goal_id).await?;
    let output_format = if json { OutputFormat::Json } else { format };

    match output_format {
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
    let project_dir = std::env::current_dir()?;
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
    let project_dir = std::env::current_dir()?;
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
    let project_dir = std::env::current_dir()?;
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
