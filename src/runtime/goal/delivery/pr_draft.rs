use anyhow::{Context, Result};

use super::{
    GoalDeliveryPolicy, GoalGithubPrClient, GoalGithubPrDeliveryOptions,
    GoalGithubPrDeliveryOutcome, GoalGithubPrOperation, GoalGithubPrRequest,
};
use crate::runtime::goal::open_pr::{render_goal_open_pr, GoalOpenPrDraft};

pub async fn open_goal_pr_with_client<C>(
    goal_id: &str,
    options: GoalGithubPrDeliveryOptions,
    client: &mut C,
) -> Result<GoalGithubPrDeliveryOutcome>
where
    C: GoalGithubPrClient,
{
    let render_as_draft = options.draft || options.policy == GoalDeliveryPolicy::DraftPr;
    let draft = render_goal_open_pr(goal_id, render_as_draft, options.dry_run).await?;
    deliver_goal_open_pr_with_client(&draft, options, client).await
}

pub async fn deliver_goal_open_pr_with_client<C>(
    draft: &GoalOpenPrDraft,
    options: GoalGithubPrDeliveryOptions,
    client: &mut C,
) -> Result<GoalGithubPrDeliveryOutcome>
where
    C: GoalGithubPrClient,
{
    if options.dry_run {
        return Ok(skipped_outcome(
            &options,
            draft.existing_pr_url.clone(),
            "dry-run delivery requested",
        ));
    }
    if !options.policy.permits_github_mutation() {
        return Ok(skipped_outcome(
            &options,
            draft.existing_pr_url.clone(),
            "local delivery policy does not permit GitHub mutation",
        ));
    }

    let request = github_request_from_draft(draft, &options)?;
    let operation = if request.existing_pr_url.is_some() {
        GoalGithubPrOperation::Update
    } else {
        GoalGithubPrOperation::Create
    };
    let mutation = match operation {
        GoalGithubPrOperation::Create => client.create_pr(request).await?,
        GoalGithubPrOperation::Update => client.update_pr(request).await?,
    };

    Ok(GoalGithubPrDeliveryOutcome {
        policy: options.policy,
        dry_run: options.dry_run,
        mutated: true,
        operation: Some(mutation.operation),
        pr_url: mutation.url,
        reason: format!("GitHub PR {} completed", mutation.operation.as_str()),
    })
}

fn github_request_from_draft(
    draft: &GoalOpenPrDraft,
    options: &GoalGithubPrDeliveryOptions,
) -> Result<GoalGithubPrRequest> {
    let head_branch = draft
        .head_branch
        .as_deref()
        .filter(|branch| !branch.trim().is_empty())
        .context("GitHub PR delivery requires a head branch")?
        .to_string();
    Ok(GoalGithubPrRequest {
        title: draft.title.clone(),
        body: draft.body.clone(),
        head_branch,
        base_branch: options.base_branch.clone(),
        draft: options.draft || options.policy == GoalDeliveryPolicy::DraftPr,
        existing_pr_url: draft.existing_pr_url.clone(),
    })
}

fn skipped_outcome(
    options: &GoalGithubPrDeliveryOptions,
    pr_url: Option<String>,
    reason: &str,
) -> GoalGithubPrDeliveryOutcome {
    GoalGithubPrDeliveryOutcome {
        policy: options.policy,
        dry_run: options.dry_run,
        mutated: false,
        operation: None,
        pr_url,
        reason: reason.to_string(),
    }
}
