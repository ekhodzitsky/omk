//! Concrete `cmd_*` handlers and output rendering for `omk goal`.
//!
//! Pure I/O -- no parsing, no validation. By the time we reach any function
//! here, `validate::*` has already normalized inputs. Budget rendering lives
//! in the `budget` submodule to keep this file under the 400-line limit.

mod budget;
mod integration;

pub(super) use budget::{cmd_budget, cmd_budget_add};
pub(super) use integration::{cmd_accept, cmd_reject};

use anyhow::{Context, Result};
use std::path::PathBuf;

use super::{OpenPrFormat, OutputFormat};

pub(super) async fn cmd_run(
    goal: &str,
    options: crate::runtime::goal::CreateGoalOptions,
) -> Result<()> {
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

pub(super) async fn cmd_plan(goal: &str) -> Result<()> {
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

pub(super) async fn cmd_list() -> Result<()> {
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

pub(super) async fn cmd_status(goal_id: &str) -> Result<()> {
    let goal = crate::runtime::goal::resolve_goal(goal_id).await?;
    println!("Goal status -- {}", goal.goal_id);
    println!("  Status:  {}", goal.status);
    println!("  Phase:   {}", goal.phase);
    println!("  Goal:    {}", goal.original_goal);
    println!("  Updated: {}", goal.updated_at);
    Ok(())
}

pub(super) async fn cmd_show(goal_id: &str, format: OutputFormat) -> Result<()> {
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

pub(super) async fn cmd_proof(goal_id: &str, format: OutputFormat) -> Result<()> {
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

pub(super) async fn cmd_open_pr(
    goal_id: &str,
    dry_run: bool,
    draft: bool,
    format: OpenPrFormat,
) -> Result<()> {
    if !dry_run {
        anyhow::bail!(
            "`omk goal open-pr` only supports dry-run rendering in this release.\nNext: omk goal open-pr {goal_id} --dry-run"
        );
    }

    let draft = crate::runtime::goal::render_goal_open_pr(goal_id, draft).await?;

    match format {
        OpenPrFormat::Json => println!("{}", serde_json::to_string_pretty(&draft)?),
        OpenPrFormat::Markdown => {
            println!("Title: {}", draft.title);
            println!("Dry-run: {}", draft.dry_run);
            println!("Draft: {}", draft.draft);
            println!();
            print!("{}", draft.body);
        }
        OpenPrFormat::Text => {
            println!("PR title: {}", draft.title);
            println!("Dry-run: {}", draft.dry_run);
            println!("Draft: {}", draft.draft);
            println!("PR body:");
            print!("{}", draft.body);
        }
    }

    Ok(())
}

pub(super) async fn cmd_replay(goal_id: &str, format: OutputFormat) -> Result<()> {
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

pub(super) async fn cmd_verify(goal_id: &str) -> Result<()> {
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

pub(super) async fn cmd_execute(goal_id: &str) -> Result<()> {
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

pub(super) async fn cmd_review(goal_id: &str) -> Result<()> {
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

fn project_dir_for_goal() -> Result<PathBuf> {
    std::env::current_dir().with_context(|| {
        "failed to read current working directory.\n\
         Run this command from the project root you want to verify, or `cd` into a readable directory."
    })
}

fn print_task_status(task_graph: &crate::runtime::goal::GoalTaskGraph, task_id: &str) {
    if let Some(task) = task_graph.tasks.iter().find(|task| task.id == task_id) {
        println!("{}: {}", task.id, task.status);
    }
}

pub(super) async fn cmd_cancel(goal_id: &str) -> Result<()> {
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

pub(super) async fn cmd_pause(goal_id: &str) -> Result<()> {
    let goal = crate::runtime::goal::pause_goal(goal_id).await?;
    println!("Goal {} paused", goal.goal_id);
    println!("Status: {}", goal.status);
    println!("Phase: {}", goal.phase);
    println!("Updated: {}", goal.updated_at);
    println!();
    println!("Resume with: omk goal resume {}", goal.goal_id);
    Ok(())
}

pub(super) async fn cmd_resume(goal_id: &str) -> Result<()> {
    let goal = crate::runtime::goal::resume_goal(goal_id).await?;
    println!("Goal {} resumed", goal.goal_id);
    println!("Status: {}", goal.status);
    println!("Phase: {}", goal.phase);
    println!("Updated: {}", goal.updated_at);
    Ok(())
}
