use serde::{Deserialize, Serialize};
use std::future::Future;
use std::pin::Pin;

mod auto_merge;
mod github_api;
mod pr_client;
mod pr_draft;
mod slice_pr;

pub use auto_merge::{attempt_auto_merge, AutoMergeAction, AutoMergeContext};
pub use github_api::{ensure_branch_protection, parse_github_owner_repo, BranchProtectionPolicy};
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn goal_delivery_policy_default_is_local() {
        assert_eq!(GoalDeliveryPolicy::default(), GoalDeliveryPolicy::Local);
    }

    #[test]
    fn goal_delivery_policy_as_str() {
        assert_eq!(GoalDeliveryPolicy::Local.as_str(), "local");
        assert_eq!(GoalDeliveryPolicy::DraftPr.as_str(), "draft-pr");
        assert_eq!(GoalDeliveryPolicy::AutoPr.as_str(), "auto-pr");
    }

    #[test]
    fn goal_delivery_policy_permits_github_mutation() {
        assert!(!GoalDeliveryPolicy::Local.permits_github_mutation());
        assert!(GoalDeliveryPolicy::DraftPr.permits_github_mutation());
        assert!(GoalDeliveryPolicy::AutoPr.permits_github_mutation());
    }

    #[test]
    fn goal_merge_policy_default_is_disabled() {
        assert_eq!(GoalMergePolicy::default(), GoalMergePolicy::Disabled);
    }

    #[test]
    fn goal_merge_policy_as_str() {
        assert_eq!(GoalMergePolicy::Disabled.as_str(), "disabled");
        assert_eq!(GoalMergePolicy::Manual.as_str(), "manual");
        assert_eq!(GoalMergePolicy::Gated.as_str(), "gated");
    }

    #[test]
    fn goal_merge_policy_permits_merge() {
        assert!(!GoalMergePolicy::Disabled.permits_merge());
        assert!(!GoalMergePolicy::Manual.permits_merge());
        assert!(GoalMergePolicy::Gated.permits_merge());
    }

    #[test]
    fn goal_github_pr_operation_as_str() {
        assert_eq!(GoalGithubPrOperation::Create.as_str(), "create");
        assert_eq!(GoalGithubPrOperation::Update.as_str(), "update");
    }

    #[test]
    fn goal_github_pr_request_serde_roundtrip() {
        let req = GoalGithubPrRequest {
            title: "Title".to_string(),
            body: "Body".to_string(),
            head_branch: "feature".to_string(),
            base_branch: Some("main".to_string()),
            draft: true,
            existing_pr_url: Some("https://github.com/example/repo/pull/1".to_string()),
        };
        let json = serde_json::to_string(&req).unwrap();
        let decoded: GoalGithubPrRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(req, decoded);
    }

    #[test]
    fn goal_github_pr_mutation_serde_roundtrip() {
        let mut_obj = GoalGithubPrMutation {
            operation: GoalGithubPrOperation::Create,
            url: Some("https://github.com/example/repo/pull/1".to_string()),
        };
        let json = serde_json::to_string(&mut_obj).unwrap();
        let decoded: GoalGithubPrMutation = serde_json::from_str(&json).unwrap();
        assert_eq!(mut_obj, decoded);
    }

    #[test]
    fn goal_github_pr_delivery_options_serde_roundtrip() {
        let opts = GoalGithubPrDeliveryOptions {
            policy: GoalDeliveryPolicy::DraftPr,
            dry_run: true,
            draft: true,
            base_branch: Some("develop".to_string()),
        };
        let json = serde_json::to_string(&opts).unwrap();
        let decoded: GoalGithubPrDeliveryOptions = serde_json::from_str(&json).unwrap();
        assert_eq!(opts, decoded);
    }

    #[test]
    fn goal_github_pr_delivery_outcome_creation() {
        let outcome = GoalGithubPrDeliveryOutcome {
            policy: GoalDeliveryPolicy::AutoPr,
            dry_run: false,
            mutated: true,
            operation: Some(GoalGithubPrOperation::Create),
            pr_url: Some("https://github.com/example/repo/pull/1".to_string()),
            reason: "ok".to_string(),
        };
        assert!(outcome.mutated);
        assert_eq!(outcome.operation, Some(GoalGithubPrOperation::Create));
    }

    #[test]
    fn goal_github_pr_command_args_creation() {
        let args = GoalGithubPrCommandArgs {
            label: "test",
            args: vec!["arg1".to_string(), "arg2".to_string()],
        };
        assert_eq!(args.label, "test");
        assert_eq!(args.args.len(), 2);
    }

    struct MockPrClient {
        next_result: Option<anyhow::Result<GoalGithubPrMutation>>,
    }

    impl GoalGithubPrClient for MockPrClient {
        fn create_pr<'a>(&'a mut self, _request: GoalGithubPrRequest) -> GoalGithubPrFuture<'a> {
            Box::pin(async move {
                self.next_result.take().unwrap_or_else(|| {
                    Ok(GoalGithubPrMutation {
                        operation: GoalGithubPrOperation::Create,
                        url: Some("https://example.com".to_string()),
                    })
                })
            })
        }

        fn update_pr<'a>(&'a mut self, _request: GoalGithubPrRequest) -> GoalGithubPrFuture<'a> {
            Box::pin(async move {
                Ok(GoalGithubPrMutation {
                    operation: GoalGithubPrOperation::Update,
                    url: Some("https://example.com".to_string()),
                })
            })
        }

        fn merge_pr<'a>(&'a mut self, _pr_url: &'a str) -> GoalGithubPrFuture<'a> {
            Box::pin(async move {
                Ok(GoalGithubPrMutation {
                    operation: GoalGithubPrOperation::Create,
                    url: Some("https://example.com".to_string()),
                })
            })
        }
    }

    #[tokio::test]
    async fn goal_github_pr_client_trait_mock_works() {
        let mut client = MockPrClient { next_result: None };
        let request = GoalGithubPrRequest {
            title: "t".to_string(),
            body: "b".to_string(),
            head_branch: "h".to_string(),
            base_branch: None,
            draft: false,
            existing_pr_url: None,
        };
        let result = client.create_pr(request.clone()).await.unwrap();
        assert_eq!(result.operation, GoalGithubPrOperation::Create);
        let result = client.update_pr(request.clone()).await.unwrap();
        assert_eq!(result.operation, GoalGithubPrOperation::Update);
        let result = client.merge_pr("url").await.unwrap();
        assert_eq!(result.url, Some("https://example.com".to_string()));
    }
}
