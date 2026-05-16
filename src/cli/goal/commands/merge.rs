//! `omk goal merge` command handler.

use anyhow::{Context, Result};

use crate::runtime::goal::{
    load_goal_task_delivery_records, resolve_goal, GoalGithubPrClient, GoalGithubPrCommandClient,
};

pub(crate) async fn cmd_merge(goal_id: &str) -> Result<()> {
    let state = resolve_goal(goal_id).await?;
    let records = load_goal_task_delivery_records(&state.state_dir).await?;

    let pr_url = records
        .into_iter()
        .filter_map(|record| {
            record
                .metadata
                .pr_url
                .or_else(|| record.metadata.extra.get("pr_link").and_then(|v| v.as_str()).map(String::from))
        })
        .next()
        .with_context(|| format!("No pr_url or pr_link found in delivery metadata for goal {}", state.goal_id))?;

    let mut client = GoalGithubPrCommandClient::default();
    client.merge_pr(&pr_url).await?;

    println!("Merged: {pr_url}");
    Ok(())
}
