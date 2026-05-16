use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;
use tokio::process::Command;

use super::open_pr::{render_goal_open_pr, GoalOpenPrDraft};

pub type GoalGithubPrFuture<'a> =
    Pin<Box<dyn Future<Output = Result<GoalGithubPrMutation>> + Send + 'a>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GoalDeliveryPolicy {
    Local,
    DraftPr,
    AutoPr,
}

impl GoalDeliveryPolicy {
    pub fn as_str(self) -> &'static str {
        match self {
            GoalDeliveryPolicy::Local => "local",
            GoalDeliveryPolicy::DraftPr => "draft-pr",
            GoalDeliveryPolicy::AutoPr => "auto-pr",
        }
    }

    fn permits_github_mutation(self) -> bool {
        matches!(
            self,
            GoalDeliveryPolicy::DraftPr | GoalDeliveryPolicy::AutoPr
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum GoalMergePolicy {
    #[default]
    Disabled,
    Manual,
    Gated,
}

impl GoalMergePolicy {
    pub fn as_str(self) -> &'static str {
        match self {
            GoalMergePolicy::Disabled => "disabled",
            GoalMergePolicy::Manual => "manual",
            GoalMergePolicy::Gated => "gated",
        }
    }

    pub fn permits_merge(self) -> bool {
        matches!(self, GoalMergePolicy::Gated)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GoalGithubPrOperation {
    Create,
    Update,
}

impl GoalGithubPrOperation {
    fn as_str(self) -> &'static str {
        match self {
            GoalGithubPrOperation::Create => "create",
            GoalGithubPrOperation::Update => "update",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoalGithubPrRequest {
    pub title: String,
    pub body: String,
    pub head_branch: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_branch: Option<String>,
    pub draft: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub existing_pr_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoalGithubPrMutation {
    pub operation: GoalGithubPrOperation,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

pub trait GoalGithubPrClient {
    fn create_pr<'a>(&'a mut self, request: GoalGithubPrRequest) -> GoalGithubPrFuture<'a>;

    fn update_pr<'a>(&'a mut self, request: GoalGithubPrRequest) -> GoalGithubPrFuture<'a>;

    fn merge_pr<'a>(&'a mut self, pr_url: &'a str) -> GoalGithubPrFuture<'a>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoalGithubPrDeliveryOptions {
    pub policy: GoalDeliveryPolicy,
    pub dry_run: bool,
    pub draft: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_branch: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoalGithubPrDeliveryOutcome {
    pub policy: GoalDeliveryPolicy,
    pub dry_run: bool,
    pub mutated: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation: Option<GoalGithubPrOperation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pr_url: Option<String>,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub struct GoalGithubPrCommandClient {
    timeout: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GoalGithubPrCommandArgs {
    label: &'static str,
    args: Vec<String>,
}

impl Default for GoalGithubPrCommandClient {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(120),
        }
    }
}

impl GoalGithubPrCommandClient {
    pub fn new(timeout: Duration) -> Self {
        Self { timeout }
    }

    async fn run_gh_pr(
        &self,
        operation: GoalGithubPrOperation,
        request: GoalGithubPrRequest,
    ) -> Result<GoalGithubPrMutation> {
        let mut pr_url = None;
        for step in github_pr_command_plan(operation, &request)? {
            let mut command = Command::new("gh");
            command.arg("pr").args(&step.args);
            let output = tokio::time::timeout(self.timeout, command.output())
                .await
                .with_context(|| format!("timed out waiting for gh pr {}", step.label))?
                .with_context(|| format!("failed to start gh pr {}", step.label))?;

            if !output.status.success() {
                anyhow::bail!(
                    "gh pr {} failed: {}",
                    step.label,
                    String::from_utf8_lossy(&output.stderr).trim()
                );
            }

            let stdout = String::from_utf8_lossy(&output.stdout);
            pr_url = pr_url.or_else(|| pr_url_from_stdout(&stdout));
        }

        Ok(GoalGithubPrMutation {
            operation,
            url: pr_url.or(request.existing_pr_url),
        })
    }
}

impl GoalGithubPrClient for GoalGithubPrCommandClient {
    fn create_pr<'a>(&'a mut self, request: GoalGithubPrRequest) -> GoalGithubPrFuture<'a> {
        Box::pin(async move { self.run_gh_pr(GoalGithubPrOperation::Create, request).await })
    }

    fn update_pr<'a>(&'a mut self, request: GoalGithubPrRequest) -> GoalGithubPrFuture<'a> {
        Box::pin(async move { self.run_gh_pr(GoalGithubPrOperation::Update, request).await })
    }

    fn merge_pr<'a>(&'a mut self, pr_url: &'a str) -> GoalGithubPrFuture<'a> {
        Box::pin(async move {
            let mut command = Command::new("gh");
            command
                .arg("pr")
                .arg("merge")
                .arg(pr_url)
                .arg("--squash")
                .arg("--delete-branch");
            let output = tokio::time::timeout(self.timeout, command.output())
                .await
                .with_context(|| format!("timed out waiting for gh pr merge {pr_url}"))?
                .with_context(|| format!("failed to start gh pr merge {pr_url}"))?;

            if !output.status.success() {
                anyhow::bail!(
                    "gh pr merge failed: {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                );
            }

            Ok(GoalGithubPrMutation {
                operation: GoalGithubPrOperation::Create,
                url: Some(pr_url.to_string()),
            })
        })
    }
}

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

fn github_pr_command_plan(
    operation: GoalGithubPrOperation,
    request: &GoalGithubPrRequest,
) -> Result<Vec<GoalGithubPrCommandArgs>> {
    match operation {
        GoalGithubPrOperation::Create => Ok(vec![create_args(request)]),
        GoalGithubPrOperation::Update => {
            let mut steps = vec![update_args(request)?];
            if let Some(draft_step) = draft_update_args(request)? {
                steps.push(draft_step);
            }
            Ok(steps)
        }
    }
}

fn create_args(request: &GoalGithubPrRequest) -> GoalGithubPrCommandArgs {
    let mut args = vec![
        "create".to_string(),
        "--title".to_string(),
        request.title.clone(),
        "--body".to_string(),
        request.body.clone(),
        "--head".to_string(),
        request.head_branch.clone(),
    ];
    if let Some(base_branch) = &request.base_branch {
        args.push("--base".to_string());
        args.push(base_branch.clone());
    }
    if request.draft {
        args.push("--draft".to_string());
    }
    GoalGithubPrCommandArgs {
        label: "create",
        args,
    }
}

fn update_args(request: &GoalGithubPrRequest) -> Result<GoalGithubPrCommandArgs> {
    let pr_url = request
        .existing_pr_url
        .as_deref()
        .context("GitHub PR update requires an existing PR URL")?;
    let mut args = vec![
        "edit".to_string(),
        pr_url.to_string(),
        "--title".to_string(),
        request.title.clone(),
        "--body".to_string(),
        request.body.clone(),
    ];
    if let Some(base_branch) = &request.base_branch {
        args.push("--base".to_string());
        args.push(base_branch.clone());
    }
    Ok(GoalGithubPrCommandArgs {
        label: "edit",
        args,
    })
}

fn draft_update_args(request: &GoalGithubPrRequest) -> Result<Option<GoalGithubPrCommandArgs>> {
    if !request.draft {
        return Ok(None);
    }
    let pr_url = request
        .existing_pr_url
        .as_deref()
        .context("GitHub draft update requires an existing PR URL")?;
    Ok(Some(GoalGithubPrCommandArgs {
        label: "ready",
        args: vec![
            "ready".to_string(),
            pr_url.to_string(),
            "--undo".to_string(),
        ],
    }))
}

fn pr_url_from_stdout(stdout: &str) -> Option<String> {
    stdout
        .lines()
        .map(str::trim)
        .find(|line| line.starts_with("http://") || line.starts_with("https://"))
        .map(str::to_string)
}

/// Poll GitHub PR required checks via `gh pr checks`.
/// Returns `Ok(true)` when all required checks pass.
/// Returns `Ok(false)` while checks are pending.
/// Returns `Err(...)` if a required check fails or the command errors.
pub async fn poll_github_pr_checks(pr_url: &str, timeout: Duration) -> Result<bool> {
    let mut command = Command::new("gh");
    command.arg("pr").arg("checks").arg(pr_url);
    let output = tokio::time::timeout(timeout, command.output())
        .await
        .with_context(|| format!("timed out waiting for gh pr checks {pr_url}"))?
        .with_context(|| format!("failed to start gh pr checks {pr_url}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("no checks reported") {
            return Ok(false);
        }
        anyhow::bail!("gh pr checks failed: {}", stderr.trim());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<_> = stdout.lines().collect();
    if lines.is_empty() {
        return Ok(false);
    }

    let mut all_pass = true;
    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        // gh pr checks output format: "check-name  pass/fail/pending  time  url"
        let parts: Vec<_> = trimmed.split_whitespace().collect();
        if parts.len() >= 2 {
            let state = parts[1];
            if state == "fail" {
                anyhow::bail!("required check '{}' failed", parts[0]);
            }
            if state != "pass" {
                all_pass = false;
            }
        }
    }
    Ok(all_pass)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(existing_pr_url: Option<&str>, draft: bool) -> GoalGithubPrRequest {
        GoalGithubPrRequest {
            title: "Title".to_string(),
            body: "Body".to_string(),
            head_branch: "codex/goal-delivery".to_string(),
            base_branch: Some("main".to_string()),
            draft,
            existing_pr_url: existing_pr_url.map(str::to_string),
        }
    }

    #[test]
    fn update_draft_pr_plan_converts_existing_pr_back_to_draft() {
        let plan = github_pr_command_plan(
            GoalGithubPrOperation::Update,
            &request(Some("https://github.com/example/repo/pull/7"), true),
        )
        .expect("draft update plan");

        assert_eq!(plan.len(), 2);
        assert_eq!(plan[0].args[0], "edit");
        assert_eq!(
            plan[1],
            GoalGithubPrCommandArgs {
                label: "ready",
                args: vec![
                    "ready".to_string(),
                    "https://github.com/example/repo/pull/7".to_string(),
                    "--undo".to_string(),
                ],
            }
        );
    }

    #[test]
    fn update_non_draft_pr_plan_only_edits_metadata() {
        let plan = github_pr_command_plan(
            GoalGithubPrOperation::Update,
            &request(Some("https://github.com/example/repo/pull/7"), false),
        )
        .expect("update plan");

        assert_eq!(plan.len(), 1);
        assert_eq!(plan[0].args[0], "edit");
    }
}
