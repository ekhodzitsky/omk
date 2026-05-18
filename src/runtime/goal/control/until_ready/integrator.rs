use anyhow::Result;
use chrono::Utc;
use serde_json::json;
use std::path::{Path, PathBuf};

use crate::runtime::events::{Event, EventKind, EventWriter, RunId};
use crate::runtime::goal::delivery::{
    poll_github_pr_checks, GoalDeliveryPolicy, GoalGithubPrClient, GoalMergePolicy,
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
use super::delivery::create_integrator_pr;
use super::gates::run_integrator_gates;
use super::git::{
    create_integrator_branch, merge_branch_into_integrator, merge_tree_is_clean, push_branch,
    resolve_base_branch,
};
use super::MANUAL_INTEGRATION_BLOCKER_FILE;

pub(crate) async fn finalize_slice_integrator(
    goal_id: &str,
    mut steps: Vec<GoalControllerStep>,
    policy: GoalDeliveryPolicy,
    merge_policy: GoalMergePolicy,
    project_dir: &Path,
) -> Result<GoalRunUntilReadyOutcome> {
    let mut state = crate::runtime::goal::resolve_goal(goal_id).await?;
    let mut proof = GoalProof::load(&state.state_dir).await?;
    let now = Utc::now();

    let records =
        crate::runtime::goal::task_graph::load_goal_task_delivery_records(&state.state_dir).await?;
    let slice_branches: Vec<String> = records
        .iter()
        .filter(|r| {
            r.metadata.status.as_ref().is_some_and(|s| {
                matches!(
                    s,
                    crate::runtime::goal::task_graph::GoalTaskDeliveryStatus::Delivered
                        | crate::runtime::goal::task_graph::GoalTaskDeliveryStatus::Merged
                )
            })
        })
        .filter_map(|r| r.metadata.branch.clone())
        .collect();

    if slice_branches.is_empty() {
        return finalize_until_ready_blocker(
            goal_id,
            steps,
            UntilReadyBlocker::policy("no delivered slices found for integrator"),
        )
        .await;
    }

    let integrator_branch = format!("integrator/{}", state.goal_id);

    let base_branch = resolve_base_branch(project_dir)
        .await
        .unwrap_or_else(|| "master".to_string());
    if let Err(e) = create_integrator_branch(project_dir, &integrator_branch, &base_branch).await {
        return finalize_until_ready_blocker(
            goal_id,
            steps,
            UntilReadyBlocker::policy(format!("integrator branch creation failed: {e}")),
        )
        .await;
    }

    for branch in &slice_branches {
        if let Err(e) = merge_tree_is_clean(project_dir, branch, &integrator_branch).await {
            return finalize_until_ready_blocker(
                goal_id,
                steps,
                UntilReadyBlocker::policy(format!(
                    "integrator merge-tree pre-check failed for branch {branch}: {e}"
                )),
            )
            .await;
        }
    }

    for branch in &slice_branches {
        if let Err(e) = merge_branch_into_integrator(project_dir, branch, &integrator_branch).await
        {
            return finalize_until_ready_blocker(
                goal_id,
                steps,
                UntilReadyBlocker::policy(format!(
                    "integrator merge failed for branch {branch}: {e}"
                )),
            )
            .await;
        }
    }

    if let Err(e) = push_branch(project_dir, &integrator_branch).await {
        return finalize_until_ready_blocker(
            goal_id,
            steps,
            UntilReadyBlocker::policy(format!("integrator push failed: {e}")),
        )
        .await;
    }

    let gate_count = match run_integrator_gates(&mut steps, project_dir, &state, &base_branch).await
    {
        Ok(count) => count,
        Err(e) => {
            return finalize_until_ready_blocker(
                goal_id,
                steps,
                UntilReadyBlocker::policy(e.to_string()),
            )
            .await;
        }
    };

    let integrator_narrative = format!(
        "integrator branch passed {} verification gate(s)",
        gate_count
    );
    let integrator_event_writer =
        EventWriter::new(state.state_dir.join(crate::runtime::config::EVENTS_FILE));
    if let Ok(event) = Event::new(RunId(state.goal_id.clone()), EventKind::TaskOutput)
        .with_actor("controller")
        .with_message(&integrator_narrative)
    {
        let _ = integrator_event_writer.append(&event).await;
    }

    let (pr_outcome, mut client) = match create_integrator_pr(
        &state,
        &slice_branches,
        &integrator_branch,
        &base_branch,
        policy,
    )
    .await
    {
        Ok(v) => v,
        Err(e) => {
            return finalize_until_ready_blocker(
                goal_id,
                steps,
                UntilReadyBlocker::policy(format!("integrator PR creation failed: {e}")),
            )
            .await;
        }
    };

    let pr_url = pr_outcome.pr_url.clone();

    let integrator_metadata = serde_json::json!({
        "integrator_branch": integrator_branch,
        "pr_url": pr_url,
        "slice_branches": slice_branches,
        "recorded_at": now,
    });
    let integrator_metadata_path = state.state_dir.join("integrator-metadata.json");
    proof::write_json_artifact(&integrator_metadata_path, &integrator_metadata).await?;
    evidence::record_artifact_path_once(
        &mut state,
        "integrator_metadata",
        integrator_metadata_path,
        now,
    );

    match merge_policy {
        GoalMergePolicy::Disabled => {
            state.status = GoalStatus::Ready;
            state.phase = GoalPhase::Proof;
            state.completed_at = Some(now);
            state.updated_at = now;
            FileSystemGoalStateStore::new().save(&state).await?;

            proof.status = GoalStatus::Ready;
            proof.readiness =
                "ready: all slices delivered; integrator PR created; merge disabled by policy"
                    .to_string();
            proof.summary = format!(
                "Goal '{}' is proof-backed ready with {} slice(s). Integrator PR created. Merge is disabled by policy.",
                state.normalized_goal,
                slice_branches.len()
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
                    "Integrator PR created under {} policy; merge disabled",
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
                "no integrator PR was created; inspect the goal state for delivery evidence"
                    .to_string()
            };
            state.status = GoalStatus::BlockedOnHuman;
            state.phase = GoalPhase::Proof;
            state.completed_at = Some(now);
            state.updated_at = now;
            FileSystemGoalStateStore::new().save(&state).await?;

            proof.status = GoalStatus::BlockedOnHuman;
            proof.readiness =
                "blocked: manual merge required before goal is fully delivered".to_string();
            proof.summary = format!(
                "Goal '{}' passed gates, execution, review, and integration evidence. Manual merge of the integrator PR is required.",
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
                "github_mutation": pr_outcome.mutated,
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

            cleanup_goal_worktrees(&state, project_dir).await;

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
                        "gated merge blocked: no integrator PR URL available for merge".to_string(),
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
                "ready: all slices delivered, integrator PR merged after gated checks".to_string();
            proof.summary = format!(
                "Goal '{}' is proof-backed ready: gates, execution, review, integration evidence passed, and integrator PR was merged after required checks passed.",
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
                    "Integrator PR created and merged under {} policy after gated checks",
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
