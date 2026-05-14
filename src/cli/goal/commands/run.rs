use anyhow::{Context, Result};

pub(crate) async fn cmd_run(
    goal: &str,
    options: crate::runtime::goal::CreateGoalOptions,
) -> Result<()> {
    if options.until_ready {
        let project_dir = std::env::current_dir()
            .context("Failed to resolve current directory for the goal controller loop")?;
        let outcome =
            crate::runtime::goal::run_goal_until_ready(goal, options, &project_dir).await?;
        print_until_ready_outcome(&outcome);
        return Ok(());
    }

    let state = crate::runtime::goal::create_goal(goal, options).await?;
    print_goal_scaffold(&state);
    Ok(())
}

fn print_goal_scaffold(state: &crate::runtime::goal::GoalState) {
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
}

fn print_until_ready_outcome(outcome: &crate::runtime::goal::GoalRunUntilReadyOutcome) {
    println!("Goal run completed: {}", outcome.state.goal_id);
    println!("  Status: {}", outcome.proof.status);
    println!("  Phase:  {}", outcome.state.phase);
    println!("  State:  {}", outcome.state.state_dir.display());
    println!(
        "  Proof:  {}",
        outcome
            .state
            .state_dir
            .join(crate::runtime::goal::GOAL_PROOF_FILE)
            .display()
    );
    println!();
    println!("Controller steps:");
    for step in &outcome.steps {
        println!(
            "  {}: {} -- {}",
            step.kind.as_str(),
            step.status,
            step.summary
        );
    }
    if let Some(blocker) = &outcome.blocker {
        println!();
        if outcome.state.status == crate::runtime::goal::GoalStatus::BlockedOnHuman {
            println!("Decision needed: {blocker}");
            println!("Next: refine the goal with testable success criteria, then run it again.");
        } else {
            println!("Blocked: {blocker}");
            println!("GitHub mutation: disabled");
            println!("Merge policy: manual");
            if let Some(path) = &outcome.policy_evidence_path {
                println!("Policy evidence: {}", path.display());
            }
        }
    }
}
