//! `omk goal open-pr` command handler.

use anyhow::Result;

use crate::cli::goal::{map_open_pr_policy, OpenPrFormat, OpenPrPolicy};
use crate::runtime::goal::{
    open_goal_pr_with_client, GoalGithubPrCommandClient, GoalGithubPrDeliveryOptions,
};

pub(crate) async fn cmd_open_pr(
    goal_id: &str,
    dry_run: bool,
    draft: bool,
    policy: OpenPrPolicy,
    base_branch: Option<String>,
    format: OpenPrFormat,
) -> Result<()> {
    let render_as_draft = draft || policy == OpenPrPolicy::DraftPr;
    let draft_obj =
        crate::runtime::goal::render_goal_open_pr(goal_id, render_as_draft, dry_run).await?;

    if dry_run || policy == OpenPrPolicy::Local {
        match format {
            OpenPrFormat::Json => println!("{}", serde_json::to_string_pretty(&draft_obj)?),
            OpenPrFormat::Markdown => {
                println!("Title: {}", draft_obj.title);
                println!("Dry-run: {}", draft_obj.dry_run);
                println!("Draft: {}", draft_obj.draft);
                println!();
                print!("{}", draft_obj.body);
            }
            OpenPrFormat::Text => {
                println!("PR title: {}", draft_obj.title);
                println!("Dry-run: {}", draft_obj.dry_run);
                println!("Draft: {}", draft_obj.draft);
                println!("PR body:");
                print!("{}", draft_obj.body);
            }
        }
        return Ok(());
    }

    let delivery_policy = map_open_pr_policy(policy);
    let mut client = GoalGithubPrCommandClient::default();
    let options = GoalGithubPrDeliveryOptions {
        policy: delivery_policy,
        dry_run: false,
        draft: render_as_draft,
        base_branch,
    };
    let outcome = open_goal_pr_with_client(goal_id, options, &mut client).await?;

    match format {
        OpenPrFormat::Json => println!("{}", serde_json::to_string_pretty(&outcome)?),
        _ => {
            println!("Policy: {}", outcome.policy.as_str());
            println!("Mutated: {}", outcome.mutated);
            if let Some(url) = &outcome.pr_url {
                println!("PR URL: {}", url);
            }
            println!("Reason: {}", outcome.reason);
        }
    }

    Ok(())
}
