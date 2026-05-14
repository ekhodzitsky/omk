use anyhow::Result;

pub(crate) async fn cmd_accept(goal_id: &str, summary: &str) -> Result<()> {
    let project_dir = std::env::current_dir()?;
    let proof = crate::runtime::goal::accept_goal(goal_id, summary, &project_dir).await?;
    print_acceptance(&proof);
    Ok(())
}

pub(crate) async fn cmd_reject(goal_id: &str, reason: &str) -> Result<()> {
    let project_dir = std::env::current_dir()?;
    let proof = crate::runtime::goal::reject_goal(goal_id, reason, &project_dir).await?;
    print_acceptance(&proof);
    Ok(())
}

fn print_acceptance(proof: &crate::runtime::goal::GoalProof) {
    println!("Acceptance: {}", proof.status);
    println!("Readiness: {}", proof.readiness);
    if !proof.known_gaps.is_empty() {
        println!("Known gaps:");
        for gap in &proof.known_gaps {
            println!("  - {gap}");
        }
    }
}
