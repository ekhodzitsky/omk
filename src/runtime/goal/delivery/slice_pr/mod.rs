use anyhow::Result;
use std::path::Path;

use super::{
    attempt_auto_merge, AutoMergeAction, AutoMergeContext, GoalDeliveryPolicy, GoalGithubPrClient,
    GoalGithubPrCommandClient, GoalGithubPrOperation, GoalGithubPrRequest,
};
use crate::runtime::goal::control::resolve_base_branch;
use crate::runtime::goal::review::{
    anti_slop_confidence_with_findings, review_slice, SliceReviewArtifact,
    ANTI_SLOP_ACTIONABLE_THRESHOLD,
};
use crate::runtime::goal::state::GoalState;
use crate::runtime::goal::task_graph::{GoalDeliverySlice, GoalTaskGraph};

mod commit;
mod git;
mod merge_check;
mod rebase;

const DEFAULT_BASE_BRANCH: &str = "main";
const DEFAULT_REMOTE: &str = "origin";

/// Options for delivering a slice PR.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SlicePrDeliveryOptions {
    pub policy: GoalDeliveryPolicy,
    pub dry_run: bool,
    pub base_branch: Option<String>,
}

/// Outcome of delivering a slice PR.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SlicePrDeliveryOutcome {
    pub commit_sha: Option<String>,
    pub pr_url: Option<String>,
    pub mutated: bool,
    pub reason: String,
    pub review_artifacts: Option<Vec<crate::runtime::goal::review::SliceReviewArtifact>>,
    pub slop_findings: Vec<crate::runtime::goal::review::slop::SlopFinding>,
    pub auto_merge_action: Option<AutoMergeAction>,
}

/// Full pipeline: detect changes → commit → push → open/update PR for one slice.
pub(crate) async fn deliver_slice_pr(
    worktree_path: &Path,
    slice: &GoalDeliverySlice,
    goal_state: &GoalState,
    task_graph: &GoalTaskGraph,
    options: SlicePrDeliveryOptions,
) -> Result<SlicePrDeliveryOutcome> {
    if options.dry_run {
        return Ok(SlicePrDeliveryOutcome {
            commit_sha: None,
            pr_url: None,
            mutated: false,
            reason: "dry-run: skipped slice PR delivery".to_string(),
            review_artifacts: None,
            slop_findings: Vec::new(),
            auto_merge_action: None,
        });
    }
    if !options.policy.permits_github_mutation() {
        return Ok(SlicePrDeliveryOutcome {
            commit_sha: None,
            pr_url: None,
            mutated: false,
            reason: "local delivery policy does not permit GitHub mutation".to_string(),
            review_artifacts: None,
            slop_findings: Vec::new(),
            auto_merge_action: None,
        });
    }

    // Check if there are any changes to commit
    let has_changes = git::git_worktree_has_changes(worktree_path).await?;
    if !has_changes {
        return Ok(SlicePrDeliveryOutcome {
            commit_sha: None,
            pr_url: None,
            mutated: false,
            reason: "no changes to commit in slice worktree".to_string(),
            review_artifacts: None,
            slop_findings: Vec::new(),
            auto_merge_action: None,
        });
    }

    let commit_sha =
        commit::commit_slice_changes(worktree_path, slice, &goal_state.goal_id).await?;

    // Run the 6-review wall before opening the PR.
    let review = review_slice(slice, goal_state, task_graph, worktree_path).await?;
    if !review.passed {
        return Ok(SlicePrDeliveryOutcome {
            commit_sha: Some(commit_sha),
            pr_url: None,
            mutated: false,
            reason: format!(
                "slice review wall blocked: {}",
                review.feedback.unwrap_or_default()
            ),
            review_artifacts: Some(review.artifacts),
            slop_findings: review.slop_findings,
            auto_merge_action: None,
        });
    }
    let anti_slop_conf =
        anti_slop_confidence_with_findings(&review.artifacts, &review.slop_findings);
    if anti_slop_conf > ANTI_SLOP_ACTIONABLE_THRESHOLD {
        return Ok(SlicePrDeliveryOutcome {
            commit_sha: Some(commit_sha),
            pr_url: None,
            mutated: false,
            reason: format!(
                "slice blocked by anti-slop confidence {:.2} exceeding threshold",
                anti_slop_conf
            ),
            review_artifacts: Some(review.artifacts),
            slop_findings: review.slop_findings,
            auto_merge_action: None,
        });
    }
    let slop_findings = review.slop_findings.clone();

    let base_branch = if let Some(ref bb) = options.base_branch {
        bb.clone()
    } else {
        resolve_base_branch(worktree_path)
            .await
            .unwrap_or_else(|| DEFAULT_BASE_BRANCH.to_string())
    };
    if let Err(e) =
        rebase::ensure_slice_branch_merge_clean(worktree_path, &slice.branch_name, &base_branch)
            .await
    {
        return Ok(SlicePrDeliveryOutcome {
            commit_sha: Some(commit_sha),
            pr_url: None,
            mutated: false,
            reason: format!("slice branch merge check failed: {e}"),
            review_artifacts: Some(review.artifacts),
            slop_findings,
            auto_merge_action: None,
        });
    }

    commit::push_slice_branch(worktree_path, &slice.branch_name).await?;

    let outcome = open_slice_pr(
        slice,
        goal_state,
        &commit_sha,
        &options,
        Some(&review.artifacts),
    )
    .await?;

    let auto_merge_action = if options.policy == GoalDeliveryPolicy::AutoPr {
        let verdict = crate::runtime::goal::review::dispatcher::aggregate_verdict(&review);
        let ctx = AutoMergeContext {
            slice_id: &slice.slice_id,
            goal_id: &goal_state.goal_id,
            pr_url: outcome.pr_url.as_deref().unwrap_or(""),
        };
        let mut client = GoalGithubPrCommandClient::default();
        Some(attempt_auto_merge(&mut client, &verdict, ctx).await)
    } else {
        None
    };

    Ok(SlicePrDeliveryOutcome {
        commit_sha: Some(commit_sha),
        pr_url: outcome.pr_url.clone(),
        mutated: outcome.mutated,
        reason: outcome.reason,
        review_artifacts: outcome.review_artifacts,
        slop_findings,
        auto_merge_action,
    })
}

/// Open or update a PR for a single slice.
async fn open_slice_pr(
    slice: &GoalDeliverySlice,
    goal_state: &GoalState,
    commit_sha: &str,
    options: &SlicePrDeliveryOptions,
    review_artifacts: Option<&[SliceReviewArtifact]>,
) -> Result<SlicePrDeliveryOutcome> {
    let head_branch = slice.branch_name.clone();
    let title = format!(
        "[slice] {} — {}",
        slice.slice_id, goal_state.normalized_goal
    );
    let body = render_slice_pr_body(slice, goal_state, commit_sha, review_artifacts);

    let request = GoalGithubPrRequest {
        title,
        body,
        head_branch,
        base_branch: options.base_branch.clone(),
        draft: options.policy == GoalDeliveryPolicy::DraftPr,
        existing_pr_url: slice.pr_url.clone(),
    };

    let mut client = GoalGithubPrCommandClient::default();
    let operation = if request.existing_pr_url.is_some() {
        GoalGithubPrOperation::Update
    } else {
        GoalGithubPrOperation::Create
    };
    let mutation = match operation {
        GoalGithubPrOperation::Create => client.create_pr(request).await?,
        GoalGithubPrOperation::Update => client.update_pr(request).await?,
    };

    Ok(SlicePrDeliveryOutcome {
        commit_sha: Some(commit_sha.to_string()),
        pr_url: mutation.url.clone(),
        mutated: true,
        reason: format!("GitHub PR {} completed", mutation.operation.as_str()),
        review_artifacts: review_artifacts.map(|a| a.to_vec()),
        slop_findings: Vec::new(),
        auto_merge_action: None,
    })
}

/// Render the PR body for a slice, including review artifact evidence.
fn render_slice_pr_body(
    slice: &GoalDeliverySlice,
    goal_state: &GoalState,
    commit_sha: &str,
    review_artifacts: Option<&[SliceReviewArtifact]>,
) -> String {
    let mut body = format!(
        "Slice `{}` for goal `{}`.\n\n- Owner: `{}`\n- Write scope: `{}`\n- Commit: `{}`\n- Slice dependencies: `{}`\n",
        slice.slice_id,
        goal_state.goal_id,
        slice.owner_role,
        slice.write_scope.join(", "),
        commit_sha,
        slice.dependencies.join(", "),
    );

    if let Some(artifacts) = review_artifacts {
        body.push_str("\n## Review Wall\n\n");
        body.push_str("| Pass | Status | Severity | Feedback |\n");
        body.push_str("|---|---|---|---|\n");
        for artifact in artifacts {
            let status = if artifact.passed { "passed" } else { "failed" };
            body.push_str(&format!(
                "| {} | {} | {} | {} |\n",
                artifact.kind, status, artifact.severity, artifact.feedback
            ));
        }
        body.push('\n');
    }

    body
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slice_pr_delivery_options_equality() {
        let a = SlicePrDeliveryOptions {
            policy: GoalDeliveryPolicy::DraftPr,
            dry_run: false,
            base_branch: Some("main".to_string()),
        };
        let b = SlicePrDeliveryOptions {
            policy: GoalDeliveryPolicy::DraftPr,
            dry_run: false,
            base_branch: Some("main".to_string()),
        };
        assert_eq!(a, b);
    }

    #[test]
    fn slice_pr_delivery_outcome_local_policy_skips() {
        let outcome = SlicePrDeliveryOutcome {
            commit_sha: None,
            pr_url: None,
            mutated: false,
            reason: "local delivery policy does not permit GitHub mutation".to_string(),
            review_artifacts: None,
            slop_findings: Vec::new(),
            auto_merge_action: None,
        };
        assert!(!outcome.mutated);
        assert!(outcome.commit_sha.is_none());
    }

    #[test]
    fn render_slice_pr_body_includes_review_artifacts() {
        let slice = GoalDeliverySlice {
            slice_id: "slice-1".to_string(),
            task_id: "t1".to_string(),
            owner_role: "executor".to_string(),
            read_scope: vec![],
            write_scope: vec!["src".to_string()],
            dependencies: vec![],
            branch_name: "branch".to_string(),
            worktree_name: "wt".to_string(),
            worktree_path: std::path::PathBuf::new(),
            gates: vec![],
            review_needs: vec![],
            pr_url: None,
        };
        let goal_state = GoalState {
            version: 1,
            goal_id: "goal-1".to_string(),
            original_goal: "Test goal".to_string(),
            normalized_goal: "Test goal".to_string(),
            status: crate::runtime::goal::state::GoalStatus::Running,
            phase: crate::runtime::goal::state::GoalPhase::Intake,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            completed_at: None,
            until_ready: false,
            budget_time: None,
            budget_tokens: None,
            budget_usd: None,
            max_agents: None,
            cost_tracker_path: None,
            terminal_criteria: crate::runtime::goal::state::GoalTerminalCriteria::default(),
            delivery_policy: GoalDeliveryPolicy::Local,
            merge_policy: crate::runtime::goal::GoalMergePolicy::Disabled,
            slice_execution: false,
            recovery_attempts: 0,
            artifacts: vec![],
            failure: None,
            state_dir: std::path::PathBuf::new(),
        };
        let artifacts = vec![
            SliceReviewArtifact {
                kind: "architect".to_string(),
                passed: true,
                feedback: "Architecture review passed".to_string(),
                severity: "low".to_string(),
            },
            SliceReviewArtifact {
                kind: "code".to_string(),
                passed: false,
                feedback: "Code review blocked".to_string(),
                severity: "high".to_string(),
            },
        ];
        let body = render_slice_pr_body(&slice, &goal_state, "abc123", Some(&artifacts));
        assert!(body.contains("## Review Wall"));
        assert!(body.contains("| architect | passed | low | Architecture review passed |"));
        assert!(body.contains("| code | failed | high | Code review blocked |"));
        assert!(body.contains("Slice `slice-1` for goal `goal-1`"));
        assert!(body.contains("Commit: `abc123`"));
    }

    #[test]
    fn render_slice_pr_body_omits_review_section_when_no_artifacts() {
        let slice = GoalDeliverySlice {
            slice_id: "slice-2".to_string(),
            task_id: "t2".to_string(),
            owner_role: "writer".to_string(),
            read_scope: vec![],
            write_scope: vec!["docs".to_string()],
            dependencies: vec![],
            branch_name: "branch".to_string(),
            worktree_name: "wt".to_string(),
            worktree_path: std::path::PathBuf::new(),
            gates: vec![],
            review_needs: vec![],
            pr_url: None,
        };
        let goal_state = GoalState {
            version: 1,
            goal_id: "goal-2".to_string(),
            original_goal: "Docs goal".to_string(),
            normalized_goal: "Docs goal".to_string(),
            status: crate::runtime::goal::state::GoalStatus::Running,
            phase: crate::runtime::goal::state::GoalPhase::Intake,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            completed_at: None,
            until_ready: false,
            budget_time: None,
            budget_tokens: None,
            budget_usd: None,
            max_agents: None,
            cost_tracker_path: None,
            terminal_criteria: crate::runtime::goal::state::GoalTerminalCriteria::default(),
            delivery_policy: GoalDeliveryPolicy::Local,
            merge_policy: crate::runtime::goal::GoalMergePolicy::Disabled,
            slice_execution: false,
            recovery_attempts: 0,
            artifacts: vec![],
            failure: None,
            state_dir: std::path::PathBuf::new(),
        };
        let body = render_slice_pr_body(&slice, &goal_state, "def456", None);
        assert!(!body.contains("## Review Wall"));
        assert!(body.contains("Slice `slice-2` for goal `goal-2`"));
    }
}
