use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Mutex, OnceLock};

use omk::runtime::goal::{
    aggregate_verdict, attempt_auto_merge, test_slice_review_outcome, AggregateReviewVerdict,
    AutoMergeAction, AutoMergeContext, GoalGithubPrClient, GoalGithubPrMutation,
    GoalGithubPrOperation, GoalGithubPrRequest, SliceReviewArtifact,
};

fn artifact(kind: &str, passed: bool) -> SliceReviewArtifact {
    SliceReviewArtifact {
        kind: kind.to_string(),
        passed,
        feedback: "feedback".to_string(),
        severity: "low".to_string(),
    }
}

// ---------- aggregate verdict tests ----------

#[test]
fn test_aggregate_verdict_all_passed_when_all_artifacts_pass() {
    let outcome = test_slice_review_outcome(vec![
        artifact("architect", true),
        artifact("code", true),
        artifact("test", true),
        artifact("security", true),
        artifact("performance", true),
        artifact("anti-slop", true),
    ]);
    let verdict = aggregate_verdict(&outcome);
    assert!(verdict.all_passed);
    assert!(verdict.per_pass.values().all(|&p| p));
    assert!(verdict.blocking_reason.is_none());
}

#[test]
fn test_aggregate_verdict_fails_when_one_pass_fails() {
    let outcome = test_slice_review_outcome(vec![
        artifact("architect", true),
        artifact("code", true),
        artifact("test", true),
        artifact("security", true),
        artifact("performance", true),
        artifact("anti-slop", false),
    ]);
    let verdict = aggregate_verdict(&outcome);
    assert!(!verdict.all_passed);
    let reason = verdict.blocking_reason.expect("should have reason");
    assert!(reason.contains("anti-slop"), "reason: {}", reason);
    assert!(reason.contains("failed"), "reason: {}", reason);
}

#[test]
fn test_aggregate_verdict_fails_when_required_pass_missing() {
    let outcome = test_slice_review_outcome(vec![
        artifact("architect", true),
        artifact("code", true),
        artifact("test", true),
        artifact("performance", true),
        artifact("anti-slop", true),
    ]);
    let verdict = aggregate_verdict(&outcome);
    assert!(!verdict.all_passed);
    let reason = verdict.blocking_reason.expect("should have reason");
    assert!(reason.contains("security"), "reason: {}", reason);
    assert!(reason.contains("missing artifact"), "reason: {}", reason);
}

#[test]
fn test_aggregate_verdict_ignores_unknown_artifact_kinds() {
    let outcome = test_slice_review_outcome(vec![
        artifact("architect", true),
        artifact("code", true),
        artifact("test", true),
        artifact("security", true),
        artifact("performance", true),
        artifact("anti-slop", true),
        artifact("unknown-kind", false),
    ]);
    let verdict = aggregate_verdict(&outcome);
    assert!(verdict.all_passed);
}

// ---------- mock client ----------

#[derive(Debug, Default)]
struct MockPrClient {
    merge_result: Option<anyhow::Result<GoalGithubPrMutation>>,
    merge_called: bool,
}

impl GoalGithubPrClient for MockPrClient {
    fn create_pr<'a>(
        &'a mut self,
        _request: GoalGithubPrRequest,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<GoalGithubPrMutation>> + Send + 'a>> {
        unimplemented!()
    }

    fn update_pr<'a>(
        &'a mut self,
        _request: GoalGithubPrRequest,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<GoalGithubPrMutation>> + Send + 'a>> {
        unimplemented!()
    }

    fn merge_pr<'a>(
        &'a mut self,
        _pr_url: &'a str,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<GoalGithubPrMutation>> + Send + 'a>> {
        self.merge_called = true;
        let result = self.merge_result.take().unwrap_or_else(|| {
            Ok(GoalGithubPrMutation {
                operation: GoalGithubPrOperation::Create,
                url: Some("https://github.com/example/omk/pull/7".to_string()),
            })
        });
        Box::pin(async move { result })
    }
}

// ---------- mock gh helpers ----------

static GH_MOCK_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

struct MockGhGuard {
    old_path: String,
    _tmp: tempfile::TempDir,
    _lock: std::sync::MutexGuard<'static, ()>,
}

fn setup_mock_gh(script: &str) -> MockGhGuard {
    let lock = GH_MOCK_LOCK.get_or_init(|| Mutex::new(()));
    let guard = lock.lock().unwrap();
    let tmp = tempfile::tempdir().expect("tempdir");
    let gh_path = tmp.path().join("gh");
    std::fs::write(&gh_path, script).expect("write gh mock");
    std::process::Command::new("chmod")
        .arg("+x")
        .arg(&gh_path)
        .output()
        .expect("chmod");
    let old_path = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{}", tmp.path().display(), old_path);
    std::env::set_var("PATH", &new_path);
    MockGhGuard {
        old_path,
        _tmp: tmp,
        _lock: guard,
    }
}

impl Drop for MockGhGuard {
    fn drop(&mut self) {
        std::env::set_var("PATH", &self.old_path);
    }
}

// ---------- attempt_auto_merge tests ----------

#[tokio::test]
async fn test_attempt_auto_merge_blocked_on_review_returns_blocked() {
    let verdict = AggregateReviewVerdict {
        all_passed: false,
        per_pass: BTreeMap::from([("architect".to_string(), false)]),
        blocking_reason: Some("review gate: failed: architect".to_string()),
    };
    let mut client = MockPrClient::default();
    let ctx = AutoMergeContext {
        slice_id: "slice-1",
        goal_id: "goal-1",
        pr_url: "https://github.com/example/omk/pull/7",
    };
    let action = attempt_auto_merge(&mut client, &verdict, ctx).await;
    assert!(matches!(action, AutoMergeAction::BlockedOnReview { .. }));
    assert!(!client.merge_called);
}

#[tokio::test]
async fn test_attempt_auto_merge_merged_when_review_pass_and_ci_green() {
    let _guard = setup_mock_gh(
        r#"#!/bin/sh
if [ "$2" = "checks" ]; then
    echo "ci pass 1m https://example.com"
    exit 0
fi
if [ "$2" = "merge" ]; then
    echo "https://github.com/example/omk/pull/7"
    exit 0
fi
echo "unknown command: $2" >&2
exit 1
"#,
    );
    let verdict = AggregateReviewVerdict {
        all_passed: true,
        per_pass: [
            ("architect".to_string(), true),
            ("code".to_string(), true),
            ("test".to_string(), true),
            ("security".to_string(), true),
            ("performance".to_string(), true),
            ("anti-slop".to_string(), true),
        ]
        .into_iter()
        .collect(),
        blocking_reason: None,
    };
    let mut client = MockPrClient::default();
    let ctx = AutoMergeContext {
        slice_id: "slice-1",
        goal_id: "goal-1",
        pr_url: "https://github.com/example/omk/pull/7",
    };
    let action = attempt_auto_merge(&mut client, &verdict, ctx).await;
    assert!(
        matches!(action, AutoMergeAction::Merged { .. }),
        "got {:?}",
        action
    );
    assert!(client.merge_called);
}

#[tokio::test]
async fn test_attempt_auto_merge_blocked_on_ci_when_checks_not_green() {
    let _guard = setup_mock_gh(
        r#"#!/bin/sh
if [ "$2" = "checks" ]; then
    echo "no checks reported" >&2
    exit 1
fi
exit 1
"#,
    );
    let verdict = AggregateReviewVerdict {
        all_passed: true,
        per_pass: [("architect".to_string(), true)].into_iter().collect(),
        blocking_reason: None,
    };
    let mut client = MockPrClient::default();
    let ctx = AutoMergeContext {
        slice_id: "slice-1",
        goal_id: "goal-1",
        pr_url: "https://github.com/example/omk/pull/7",
    };
    let action = attempt_auto_merge(&mut client, &verdict, ctx).await;
    assert!(
        matches!(action, AutoMergeAction::BlockedOnCi),
        "got {:?}",
        action
    );
    assert!(!client.merge_called);
}

#[tokio::test]
async fn test_attempt_auto_merge_blocked_on_ci_polling_error() {
    let _guard = setup_mock_gh(
        r#"#!/bin/sh
if [ "$2" = "checks" ]; then
    echo "real polling error" >&2
    exit 1
fi
exit 1
"#,
    );
    let verdict = AggregateReviewVerdict {
        all_passed: true,
        per_pass: [("architect".to_string(), true)].into_iter().collect(),
        blocking_reason: None,
    };
    let mut client = MockPrClient::default();
    let ctx = AutoMergeContext {
        slice_id: "slice-1",
        goal_id: "goal-1",
        pr_url: "https://github.com/example/omk/pull/7",
    };
    let action = attempt_auto_merge(&mut client, &verdict, ctx).await;
    assert!(
        matches!(action, AutoMergeAction::BlockedOnCi),
        "got {:?}",
        action
    );
    assert!(!client.merge_called);
}

#[tokio::test]
async fn test_attempt_auto_merge_handles_gh_cli_error() {
    let _guard = setup_mock_gh(
        r#"#!/bin/sh
if [ "$2" = "checks" ]; then
    echo "ci pass 1m https://example.com"
    exit 0
fi
if [ "$2" = "merge" ]; then
    echo "merge conflict" >&2
    exit 1
fi
echo "unknown command: $2" >&2
exit 1
"#,
    );
    let verdict = AggregateReviewVerdict {
        all_passed: true,
        per_pass: [("architect".to_string(), true)].into_iter().collect(),
        blocking_reason: None,
    };
    let mut client = MockPrClient {
        merge_result: Some(Err(anyhow::anyhow!("merge failed"))),
        ..Default::default()
    };
    let ctx = AutoMergeContext {
        slice_id: "slice-1",
        goal_id: "goal-1",
        pr_url: "https://github.com/example/omk/pull/7",
    };
    let action = attempt_auto_merge(&mut client, &verdict, ctx).await;
    assert!(
        matches!(action, AutoMergeAction::Error { .. }),
        "got {:?}",
        action
    );
    assert!(client.merge_called);
}
