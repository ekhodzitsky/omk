use serde::{Deserialize, Serialize};
use std::future::Future;
use std::pin::Pin;

mod pr_client;
mod pr_draft;
mod slice_pr;

pub use pr_client::{poll_github_pr_checks, GoalGithubPrCommandClient};
pub use pr_draft::{deliver_goal_open_pr_with_client, open_goal_pr_with_client};
pub(super) use slice_pr::{deliver_slice_pr, SlicePrDeliveryOptions};

pub(super) type GoalGithubPrFuture<'a> =
    Pin<Box<dyn Future<Output = anyhow::Result<GoalGithubPrMutation>> + Send + 'a>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum GoalDeliveryPolicy {
    #[default]
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

    pub(crate) fn permits_github_mutation(self) -> bool {
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
    pub fn as_str(self) -> &'static str {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GoalGithubPrCommandArgs {
    pub label: &'static str,
    pub args: Vec<String>,
}
