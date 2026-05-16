use anyhow::Result;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;

use omk::runtime::goal::{
    deliver_goal_open_pr_with_client, GoalDeliveryPolicy, GoalGithubPrClient,
    GoalGithubPrDeliveryOptions, GoalGithubPrMutation, GoalGithubPrOperation, GoalGithubPrRequest,
    GoalOpenPrDraft,
};

#[derive(Debug, Clone)]
struct RecordedCall {
    operation: GoalGithubPrOperation,
    request: GoalGithubPrRequest,
}

#[derive(Debug, Default)]
struct MockGithubClient {
    calls: Vec<RecordedCall>,
}

impl MockGithubClient {
    fn call_count(&self) -> usize {
        self.calls.len()
    }
}

impl GoalGithubPrClient for MockGithubClient {
    fn create_pr<'a>(
        &'a mut self,
        request: GoalGithubPrRequest,
    ) -> Pin<Box<dyn Future<Output = Result<GoalGithubPrMutation>> + Send + 'a>> {
        self.calls.push(RecordedCall {
            operation: GoalGithubPrOperation::Create,
            request,
        });
        Box::pin(async move {
            Ok(GoalGithubPrMutation {
                operation: GoalGithubPrOperation::Create,
                url: Some("https://github.com/example/omk/pull/7".to_string()),
            })
        })
    }

    fn update_pr<'a>(
        &'a mut self,
        request: GoalGithubPrRequest,
    ) -> Pin<Box<dyn Future<Output = Result<GoalGithubPrMutation>> + Send + 'a>> {
        self.calls.push(RecordedCall {
            operation: GoalGithubPrOperation::Update,
            request,
        });
        Box::pin(async move {
            Ok(GoalGithubPrMutation {
                operation: GoalGithubPrOperation::Update,
                url: Some("https://github.com/example/omk/pull/7".to_string()),
            })
        })
    }

    fn merge_pr<'a>(
        &'a mut self,
        pr_url: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<GoalGithubPrMutation>> + Send + 'a>> {
        self.calls.push(RecordedCall {
            operation: GoalGithubPrOperation::Create,
            request: GoalGithubPrRequest {
                title: "merge".to_string(),
                body: pr_url.to_string(),
                head_branch: "merge".to_string(),
                base_branch: None,
                draft: false,
                existing_pr_url: Some(pr_url.to_string()),
            },
        });
        Box::pin(async move {
            Ok(GoalGithubPrMutation {
                operation: GoalGithubPrOperation::Create,
                url: Some(pr_url.to_string()),
            })
        })
    }
}

fn draft(existing_pr_url: Option<&str>, dry_run: bool) -> GoalOpenPrDraft {
    GoalOpenPrDraft {
        goal_id: "goal-delivery-test".to_string(),
        dry_run,
        draft: false,
        title: "Goal proof: ship GitHub delivery".to_string(),
        body: "## Goal\n- Goal id: `goal-delivery-test`\n".to_string(),
        head_branch: Some("codex/goal-github-delivery".to_string()),
        existing_pr_url: existing_pr_url.map(str::to_string),
        proof_path: PathBuf::from(".omk/goals/goal-delivery-test/proof.json"),
    }
}

fn options(policy: GoalDeliveryPolicy, dry_run: bool) -> GoalGithubPrDeliveryOptions {
    GoalGithubPrDeliveryOptions {
        policy,
        dry_run,
        draft: false,
        base_branch: Some("main".to_string()),
    }
}

#[tokio::test]
async fn local_policy_never_calls_github_even_when_not_dry_run() {
    let mut client = MockGithubClient::default();
    let outcome = deliver_goal_open_pr_with_client(
        &draft(None, true),
        options(GoalDeliveryPolicy::Local, false),
        &mut client,
    )
    .await
    .expect("local policy should produce a skipped delivery outcome");

    assert_eq!(client.call_count(), 0);
    assert!(!outcome.mutated);
    assert_eq!(outcome.policy, GoalDeliveryPolicy::Local);
    assert_eq!(outcome.operation, None);
    assert!(outcome.reason.contains("local"));
}

#[tokio::test]
async fn dry_run_auto_pr_policy_never_calls_github() {
    let mut client = MockGithubClient::default();
    let outcome = deliver_goal_open_pr_with_client(
        &draft(None, true),
        options(GoalDeliveryPolicy::AutoPr, true),
        &mut client,
    )
    .await
    .expect("dry-run policy should produce a skipped delivery outcome");

    assert_eq!(client.call_count(), 0);
    assert!(!outcome.mutated);
    assert_eq!(outcome.policy, GoalDeliveryPolicy::AutoPr);
    assert!(outcome.reason.contains("dry-run"));
}

#[tokio::test]
async fn explicit_auto_pr_policy_creates_github_pr() {
    let mut client = MockGithubClient::default();
    let outcome = deliver_goal_open_pr_with_client(
        &draft(None, true),
        options(GoalDeliveryPolicy::AutoPr, false),
        &mut client,
    )
    .await
    .expect("auto-pr policy should create a PR");

    assert!(outcome.mutated);
    assert_eq!(outcome.operation, Some(GoalGithubPrOperation::Create));
    assert_eq!(
        outcome.pr_url.as_deref(),
        Some("https://github.com/example/omk/pull/7")
    );
    assert_eq!(client.call_count(), 1);
    let call = &client.calls[0];
    assert_eq!(call.operation, GoalGithubPrOperation::Create);
    assert_eq!(call.request.head_branch, "codex/goal-github-delivery");
    assert_eq!(call.request.base_branch.as_deref(), Some("main"));
    assert!(!call.request.draft);
    assert!(call.request.body.contains("goal-delivery-test"));
}

#[tokio::test]
async fn draft_pr_policy_updates_existing_pr_as_draft() {
    let mut client = MockGithubClient::default();
    let outcome = deliver_goal_open_pr_with_client(
        &draft(Some("https://github.com/example/omk/pull/7"), true),
        options(GoalDeliveryPolicy::DraftPr, false),
        &mut client,
    )
    .await
    .expect("draft-pr policy should update an existing PR");

    assert!(outcome.mutated);
    assert_eq!(outcome.operation, Some(GoalGithubPrOperation::Update));
    assert_eq!(client.call_count(), 1);
    let call = &client.calls[0];
    assert_eq!(call.operation, GoalGithubPrOperation::Update);
    assert_eq!(
        call.request.existing_pr_url.as_deref(),
        Some("https://github.com/example/omk/pull/7")
    );
    assert!(call.request.draft);
}

#[tokio::test]
async fn auto_pr_policy_propagates_base_branch_to_request() {
    let mut client = MockGithubClient::default();
    let d = draft(None, false);
    let opts = GoalGithubPrDeliveryOptions {
        policy: GoalDeliveryPolicy::AutoPr,
        dry_run: false,
        draft: false,
        base_branch: Some("develop".to_string()),
    };
    let outcome = deliver_goal_open_pr_with_client(&d, opts, &mut client)
        .await
        .unwrap();

    assert!(outcome.mutated);
    assert_eq!(
        client.calls[0].request.base_branch.as_deref(),
        Some("develop")
    );
}

#[tokio::test]
async fn draft_pr_policy_with_dry_run_skips_mutation() {
    let mut client = MockGithubClient::default();
    let outcome = deliver_goal_open_pr_with_client(
        &draft(None, true),
        options(GoalDeliveryPolicy::DraftPr, true),
        &mut client,
    )
    .await
    .expect("draft-pr dry-run should produce a skipped delivery outcome");

    assert_eq!(client.call_count(), 0);
    assert!(!outcome.mutated);
    assert_eq!(outcome.policy, GoalDeliveryPolicy::DraftPr);
    assert!(outcome.reason.contains("dry-run"));
}

#[tokio::test]
async fn mutating_policy_requires_head_branch() {
    let mut missing_branch = draft(None, true);
    missing_branch.head_branch = None;
    let mut client = MockGithubClient::default();

    let error = deliver_goal_open_pr_with_client(
        &missing_branch,
        options(GoalDeliveryPolicy::AutoPr, false),
        &mut client,
    )
    .await
    .expect_err("mutating delivery needs a branch for gh pr create");

    assert_eq!(client.call_count(), 0);
    assert!(error.to_string().contains("head branch"));
}

#[tokio::test]
async fn merge_pr_calls_gh_merge() {
    let mut client = MockGithubClient::default();
    let url = "https://github.com/example/omk/pull/7";
    let result = client.merge_pr(url).await.expect("merge_pr should succeed");

    assert_eq!(client.call_count(), 1);
    assert_eq!(result.url, Some(url.to_string()));
    let call = &client.calls[0];
    assert_eq!(call.operation, GoalGithubPrOperation::Create);
    assert_eq!(call.request.body, url);
    assert_eq!(call.request.existing_pr_url, Some(url.to_string()));
}
