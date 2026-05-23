use std::time::Duration;

use tracing::{info, warn};

use super::{poll_github_pr_checks, GoalGithubPrClient};
use crate::runtime::goal::review::dispatcher::AggregateReviewVerdict;

/// CI check polling timeout when waiting for green status.
const CI_POLL_TIMEOUT: Duration = Duration::from_secs(6 * 60);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AutoMergeAction {
    /// Merged successfully.
    Merged { commit_sha: Option<String> },
    /// Verdict pass but CI not green within timeout. Slice flagged
    /// in escalation log.
    BlockedOnCi,
    /// Review verdict failed. Slice blocked, escalation logged.
    BlockedOnReview { reason: String },
    /// Branch protection required-checks list mismatch or repo not
    /// configured. Slice blocked with escalation.
    BlockedOnProtection { reason: String },
    /// Auto-merge skipped because delivery policy is not AutoPr.
    SkippedNotAutoPolicy,
    /// gh CLI / network error during merge. Slice blocked.
    Error { reason: String },
}

#[derive(Debug)]
pub struct AutoMergeContext<'a> {
    pub slice_id: &'a str,
    pub goal_id: &'a str,
    pub pr_url: &'a str,
}

/// Decide and execute auto-merge.
///
/// Steps:
///   1. Check verdict.all_passed; if false → BlockedOnReview
///   2. Poll CI checks (poll_github_pr_checks); if !ok → BlockedOnCi
///   3. Merge via client.merge_pr (gh pr merge --squash --delete-branch)
///   4. Emit tracing log on every terminal state (bus and escalation_log
///      are not available in this build; see known limitations).
pub async fn attempt_auto_merge(
    client: &mut (dyn GoalGithubPrClient + Send + Sync),
    verdict: &AggregateReviewVerdict,
    ctx: AutoMergeContext<'_>,
) -> AutoMergeAction {
    if !verdict.all_passed {
        let reason = verdict
            .blocking_reason
            .clone()
            .unwrap_or_else(|| "review verdict failed".to_string());
        warn!(
            slice_id = %ctx.slice_id,
            goal_id = %ctx.goal_id,
            pr_url = %ctx.pr_url,
            reason = %reason,
            "auto-merge blocked on review"
        );
        return AutoMergeAction::BlockedOnReview { reason };
    }

    match poll_github_pr_checks(ctx.pr_url, CI_POLL_TIMEOUT).await {
        Ok(true) => {}
        Ok(false) => {
            warn!(
                slice_id = %ctx.slice_id,
                goal_id = %ctx.goal_id,
                pr_url = %ctx.pr_url,
                "auto-merge blocked on CI: checks not green"
            );
            return AutoMergeAction::BlockedOnCi;
        }
        Err(e) => {
            warn!(
                slice_id = %ctx.slice_id,
                goal_id = %ctx.goal_id,
                pr_url = %ctx.pr_url,
                error = %e,
                "auto-merge blocked on CI: polling error"
            );
            return AutoMergeAction::BlockedOnCi;
        }
    }

    match client.merge_pr(ctx.pr_url).await {
        Ok(mutation) => {
            info!(
                slice_id = %ctx.slice_id,
                goal_id = %ctx.goal_id,
                pr_url = %ctx.pr_url,
                "auto-merge succeeded"
            );
            AutoMergeAction::Merged {
                commit_sha: mutation.url,
            }
        }
        Err(e) => {
            let reason = format!("gh pr merge failed: {e}");
            warn!(
                slice_id = %ctx.slice_id,
                goal_id = %ctx.goal_id,
                pr_url = %ctx.pr_url,
                error = %e,
                "auto-merge failed"
            );
            AutoMergeAction::Error { reason }
        }
    }
}
