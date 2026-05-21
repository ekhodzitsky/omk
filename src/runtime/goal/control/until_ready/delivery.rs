use anyhow::Result;
use chrono::Utc;
use serde_json::json;
use std::path::{Path, PathBuf};

use crate::runtime::goal::delivery::{
    open_goal_pr_with_client, poll_github_pr_checks, GoalDeliveryPolicy, GoalGithubPrClient,
    GoalGithubPrCommandClient, GoalGithubPrDeliveryOptions, GoalGithubPrRequest, GoalMergePolicy,
};
use crate::runtime::goal::proof::GoalProof;
use crate::runtime::goal::state::{
    self, FileSystemGoalStateStore, GoalPhase, GoalStateStore, GoalStatus,
};
use crate::runtime::goal::types::{
    GoalControllerStep, GoalControllerStepKind, GoalRunUntilReadyOutcome,
};
use crate::runtime::goal::{evidence, proof};

use super::blocker::{finalize_until_ready_blocker, UntilReadyBlocker};
use super::cleanup_goal_worktrees;
use super::MANUAL_INTEGRATION_BLOCKER_FILE;

pub(crate) async fn finalize_until_ready_delivery(
    goal_id: &str,
    mut steps: Vec<GoalControllerStep>,
    policy: GoalDeliveryPolicy,
    merge_policy: GoalMergePolicy,
    enforce_protection: bool,
    project_dir: &Path,
) -> Result<GoalRunUntilReadyOutcome> {
    let state = crate::runtime::goal::resolve_goal(goal_id).await?;

    if state.slice_execution && policy != GoalDeliveryPolicy::Local {
        return super::integrator::finalize_slice_integrator(
            goal_id,
            steps,
            policy,
            merge_policy,
            enforce_protection,
            project_dir,
        )
        .await;
    }

    let mut state = crate::runtime::goal::resolve_goal(goal_id).await?;
    let mut proof = GoalProof::load(&state.state_dir).await?;

    let delivery_options = GoalGithubPrDeliveryOptions {
        policy,
        dry_run: false,
        draft: policy == GoalDeliveryPolicy::DraftPr,
        base_branch: None,
    };
    let mut client = GoalGithubPrCommandClient::default();
    let outcome = open_goal_pr_with_client(goal_id, delivery_options, &mut client).await?;

    let now = Utc::now();
    let pr_url = outcome.pr_url.clone();

    match merge_policy {
        GoalMergePolicy::Disabled => {
            state.status = GoalStatus::Ready;
            state.phase = GoalPhase::Proof;
            state.completed_at = Some(now);
            state.updated_at = now;
            FileSystemGoalStateStore::new().save(&state).await?;

            proof.status = GoalStatus::Ready;
            proof.readiness =
                "ready: integration and oracle evidence passed; merge disabled by policy"
                    .to_string();
            proof.summary = format!(
                "Goal '{}' is proof-backed ready: gates, execution, review, and integration evidence passed. Merge is disabled by policy.",
                state.normalized_goal
            );
            proof.generated_at = now;
            proof.artifacts = state.artifacts.clone();
            proof.known_gaps.clear();
            proof.human_decisions_required.clear();
            proof.git = crate::runtime::goal::evidence::detect_git_evidence(project_dir)
                .await
                .or(proof.git);
            proof::write_json_artifact(&state.state_dir.join(state::GOAL_PROOF_FILE), &proof)
                .await?;

            steps.push(GoalControllerStep {
                kind: GoalControllerStepKind::Deliver,
                status: GoalStatus::Ready,
                summary: format!(
                    "GitHub PR created under {} policy; merge disabled",
                    policy.as_str()
                ),
            });

            cleanup_goal_worktrees(&state, project_dir).await;

            Ok(GoalRunUntilReadyOutcome {
                state,
                proof,
                steps,
                blocker: None,
                policy_evidence_path: None,
            })
        }
        GoalMergePolicy::Manual => {
            let instruction = if let Some(ref url) = pr_url {
                format!("run `gh pr merge {url} --squash --delete-branch` after CI passes")
            } else {
                "no PR was created; inspect the goal state for delivery evidence".to_string()
            };
            cleanup_goal_worktrees(&state, project_dir).await;
            state.status = GoalStatus::BlockedOnHuman;
            state.phase = GoalPhase::Proof;
            state.completed_at = Some(now);
            state.updated_at = now;
            FileSystemGoalStateStore::new().save(&state).await?;

            proof.status = GoalStatus::BlockedOnHuman;
            proof.readiness =
                "blocked: manual merge required before goal is fully delivered".to_string();
            proof.summary = format!(
                "Goal '{}' passed gates, execution, review, and integration evidence. Manual merge is required.",
                state.normalized_goal
            );
            proof.generated_at = now;
            proof.artifacts = state.artifacts.clone();
            proof.human_decisions_required.push(instruction.clone());
            proof.git = crate::runtime::goal::evidence::detect_git_evidence(project_dir)
                .await
                .or(proof.git);
            proof::write_json_artifact(&state.state_dir.join(state::GOAL_PROOF_FILE), &proof)
                .await?;

            let artifact = json!({
                "status": "blocked",
                "reason": &instruction,
                "delivery_policy": policy.as_str(),
                "merge_policy": merge_policy.as_str(),
                "github_mutation": outcome.mutated,
                "integrator_acceptance": "manual",
                "proof": state::GOAL_PROOF_FILE,
                "recorded_at": now,
            });
            proof::write_json_artifact(
                &state.state_dir.join(MANUAL_INTEGRATION_BLOCKER_FILE),
                &artifact,
            )
            .await?;
            evidence::record_artifact_path_once(
                &mut state,
                "policy_blocker",
                PathBuf::from(MANUAL_INTEGRATION_BLOCKER_FILE),
                now,
            );

            steps.push(GoalControllerStep {
                kind: GoalControllerStepKind::Blocked,
                status: GoalStatus::BlockedOnHuman,
                summary: instruction.clone(),
            });

            Ok(GoalRunUntilReadyOutcome {
                state,
                proof,
                steps,
                blocker: Some(instruction),
                policy_evidence_path: Some(PathBuf::from(MANUAL_INTEGRATION_BLOCKER_FILE)),
            })
        }
        GoalMergePolicy::Gated => {
            if let Err(e) = proof.validate_for_merge() {
                return finalize_until_ready_blocker(
                    goal_id,
                    steps,
                    UntilReadyBlocker::policy(format!(
                        "gated merge blocked: proof validation failed: {e}"
                    )),
                )
                .await;
            }
            if pr_url.is_none() {
                return finalize_until_ready_blocker(
                    goal_id,
                    steps,
                    UntilReadyBlocker::policy(
                        "gated merge blocked: no PR URL available for merge".to_string(),
                    ),
                )
                .await;
            }
            if let Some(ref url) = pr_url {
                let check_timeout = std::time::Duration::from_secs(120);
                let poll_interval = std::time::Duration::from_secs(10);
                let mut checks_pass = false;
                for _ in 0..36 {
                    match poll_github_pr_checks(url, check_timeout).await {
                        Ok(true) => {
                            checks_pass = true;
                            break;
                        }
                        Ok(false) => {
                            tokio::time::sleep(poll_interval).await;
                            continue;
                        }
                        Err(e) => {
                            return finalize_until_ready_blocker(
                                goal_id,
                                steps,
                                UntilReadyBlocker::policy(format!(
                                    "gated merge blocked: required CI check failed: {e}"
                                )),
                            )
                            .await;
                        }
                    }
                }
                if !checks_pass {
                    return finalize_until_ready_blocker(
                        goal_id,
                        steps,
                        UntilReadyBlocker::policy(
                            "gated merge blocked: required CI checks did not pass within timeout"
                                .to_string(),
                        ),
                    )
                    .await;
                }
                if let Err(e) = client.merge_pr(url).await {
                    return finalize_until_ready_blocker(
                        goal_id,
                        steps,
                        UntilReadyBlocker::policy(format!(
                            "gated merge blocked: PR merge failed: {e}"
                        )),
                    )
                    .await;
                }
            }

            state.status = GoalStatus::Ready;
            state.phase = GoalPhase::Proof;
            state.completed_at = Some(now);
            state.updated_at = now;
            FileSystemGoalStateStore::new().save(&state).await?;

            proof.status = GoalStatus::Ready;
            proof.readiness =
                "ready: integration and oracle evidence passed, GitHub PR merged after gated checks"
                    .to_string();
            proof.summary = format!(
                "Goal '{}' is proof-backed ready: gates, execution, review, integration evidence passed, and PR was merged after required checks passed.",
                state.normalized_goal
            );
            proof.generated_at = now;
            proof.artifacts = state.artifacts.clone();
            proof.known_gaps.clear();
            proof.human_decisions_required.clear();
            proof.git = crate::runtime::goal::evidence::detect_git_evidence(project_dir)
                .await
                .or(proof.git);
            proof::write_json_artifact(&state.state_dir.join(state::GOAL_PROOF_FILE), &proof)
                .await?;

            steps.push(GoalControllerStep {
                kind: GoalControllerStepKind::Deliver,
                status: GoalStatus::Ready,
                summary: format!(
                    "GitHub PR created and merged under {} policy after gated checks",
                    policy.as_str()
                ),
            });

            cleanup_goal_worktrees(&state, project_dir).await;

            Ok(GoalRunUntilReadyOutcome {
                state,
                proof,
                steps,
                blocker: None,
                policy_evidence_path: None,
            })
        }
    }
}

pub(super) async fn create_integrator_pr(
    state: &state::GoalState,
    slice_branches: &[String],
    integrator_branch: &str,
    base_branch: &str,
    policy: GoalDeliveryPolicy,
) -> Result<(
    crate::runtime::goal::delivery::GoalGithubPrDeliveryOutcome,
    GoalGithubPrCommandClient,
)> {
    let title = format!("[Integrator] {}", state.normalized_goal);
    let body = format!(
        "Integrator PR combining {} slice(s) for goal `{}`.\n\nSlices:\n{}",
        slice_branches.len(),
        state.goal_id,
        slice_branches
            .iter()
            .map(|b| format!("- `{}`", b))
            .collect::<Vec<_>>()
            .join("\n")
    );

    let pr_request = GoalGithubPrRequest {
        title,
        body,
        head_branch: integrator_branch.to_string(),
        base_branch: Some(base_branch.to_string()),
        draft: policy == GoalDeliveryPolicy::DraftPr,
        existing_pr_url: None,
    };

    let mut client = GoalGithubPrCommandClient::default();
    let pr_outcome = match client.create_pr(pr_request).await {
        Ok(mutation) => crate::runtime::goal::delivery::GoalGithubPrDeliveryOutcome {
            policy,
            dry_run: false,
            mutated: true,
            operation: Some(mutation.operation),
            pr_url: mutation.url.clone(),
            reason: format!("GitHub PR {} completed", mutation.operation.as_str()),
        },
        Err(e) => anyhow::bail!("integrator PR creation failed: {e}"),
    };

    Ok((pr_outcome, client))
}
