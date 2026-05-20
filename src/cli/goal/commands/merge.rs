use anyhow::{Context, Result};

use crate::runtime::goal::{
    merge_goal, GoalGithubPrCommandClient,
};

pub(in crate::cli::goal) async fn cmd_merge(goal_id: &str) -> Result<()> {
    let mut client = GoalGithubPrCommandClient::default();
    let state = merge_goal(goal_id, &mut client)
        .await
        .with_context(|| format!("Failed to merge goal {goal_id}"))?;

    let pr_url = state
        .artifacts
        .iter()
        .find(|a| a.kind == "pr_merge")
        .map(|a| a.path.display().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    println!(
        "Successfully merged PR for goal {}: {}",
        state.goal_id, pr_url
    );
    println!(
        "Run 'omk goal show {}' to inspect the updated state.",
        state.goal_id
    );

    Ok(())
}
