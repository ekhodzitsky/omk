use anyhow::{Context, Result};
use chrono::Utc;
use serde_json::json;
use std::path::{Path, PathBuf};

use crate::runtime::goal::delivery::{
    open_goal_pr_with_client, poll_github_pr_checks, GoalDeliveryPolicy, GoalGithubPrClient,
    GoalGithubPrCommandClient, GoalGithubPrDeliveryOptions, GoalMergePolicy,
};
use crate::runtime::goal::proof::GoalProof;
use crate::runtime::goal::state::{
    self, FileSystemGoalStateStore, GoalPhase, GoalStateStore, GoalStatus,
};
use crate::runtime::goal::task_graph::{
    all_slices_done, goal_task_done, GoalTaskGraph, GoalTaskStatus,
};
use crate::runtime::goal::types::{
    GoalControllerStep, GoalControllerStepKind, GoalRunUntilReadyOutcome,
};
use crate::runtime::goal::worktree::remove_goal_worktrees;
use crate::runtime::goal::{evidence, proof};
use crate::runtime::events::{Event, EventBuilder, EventKind, EventWriter, RunId};

const MAX_EXECUTE_PASSES: usize = 8;
const GIT_COMMAND_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);
const MANUAL_INTEGRATION_BLOCKER_FILE: &str = "artifacts/policy/manual-integration-blocker.json";
const UNTIL_READY_BLOCKER_FILE: &str = "artifacts/policy/until-ready-blocker.json";

#[derive(Debug, Clone)]
struct UntilReadyBlocker {
    reason: String,
    artifact_file: &'static str,
    human_decision_required: bool,
}

impl UntilReadyBlocker {
    fn policy(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
            artifact_file: UNTIL_READY_BLOCKER_FILE,
            human_decision_required: false,
        }
    }

    fn human(reason: impl Into<String>, artifact_file: &'static str) -> Self {
        Self {
            reason: reason.into(),
            artifact_file,
            human_decision_required: true,
        }
    }
}

pub(crate) async fn run_goal_until_ready(
    goal: &str,
    options: state::CreateGoalOptions,
    project_dir: &Path,
) -> Result<GoalRunUntilReadyOutcome> {
    let state = crate::runtime::goal::create_goal(goal, options.clone()).await?;
    let events_path = state.state_dir.join(crate::runtime::config::EVENTS_FILE);
    let event_writer = EventWriter::new(events_path);
    let event_builder = EventBuilder::new(RunId(state.goal_id.clone()));

    let plan_step = GoalControllerStep {
        kind: GoalControllerStepKind::Plan,
        status: state.status,
        summary: "created durable goal scaffold and planning artifacts".to_string(),
    };
    emit_narrative(&event_writer, &event_builder, &RunId(state.goal_id.clone()), &plan_step.summary).await;
    let mut steps = vec![plan_step];

    let proof = GoalProof::load(&state.state_dir).await?;
    if state.status == GoalStatus::BlockedOnHuman {
        let blocker = state
            .failure
            .as_ref()
            .map(|failure| failure.reason.clone())
            .unwrap_or_else(|| "human decision required before execution".to_string());
        steps.push(GoalControllerStep {
            kind: GoalControllerStepKind::Blocked,
            status: state.status,
            summary: blocker.clone(),
        });
        emit_narrative(&event_writer, &event_builder, &RunId(state.goal_id.clone()), &format!("blocked: {blocker}")).await;
        return Ok(GoalRunUntilReadyOutcome {
            state,
            proof,
            steps,
            blocker: Some(blocker),
            policy_evidence_path: None,
        });
    }

    let verified = crate::runtime::goal::verify_goal(&state.goal_id, project_dir).await?;
    let verify_summary = verification_summary(&verified);
    steps.push(GoalControllerStep {
        kind: GoalControllerStepKind::Verify,
        status: verified.status,
        summary: verify_summary.clone(),
    });
    emit_narrative(&event_writer, &event_builder, &RunId(state.goal_id.clone()), &format!("verify: {verify_summary}")).await;
    if !verification_can_continue(&verified) {
        return finalize_until_ready_blocker(
            &state.goal_id,
            steps,
            UntilReadyBlocker::policy(verification_blocker(&verified)),
        )
        .await;
    }

    for pass in 1..=MAX_EXECUTE_PASSES {
        let executed = crate::runtime::goal::execute_goal(&state.goal_id, project_dir).await?;
        let exec_summary = format!("execution pass {pass}: {}", executed.summary);
        steps.push(GoalControllerStep {
            kind: GoalControllerStepKind::Execute,
            status: executed.status,
            summary: exec_summary.clone(),
        });
        emit_narrative(&event_writer, &event_builder, &RunId(state.goal_id.clone()), &exec_summary).await;
        if !proof_can_continue(&executed) {
            return finalize_until_ready_blocker(
                &state.goal_id,
                steps,
                terminal_blocker(&executed),
            )
            .await;
        }
        if !has_pending_agent_dispatch(&state.goal_id).await? {
            break;
        }
        if pass == MAX_EXECUTE_PASSES {
            return finalize_until_ready_blocker(
                &state.goal_id,
                steps,
                UntilReadyBlocker::policy(
                    "execution stopped after the maximum controller follow-up passes; inspect pending task graph follow-ups",
                ),
            )
            .await;
        }
    }

    let reviewed = crate::runtime::goal::review_goal(&state.goal_id, project_dir).await?;
    steps.push(GoalControllerStep {
        kind: GoalControllerStepKind::Review,
        status: reviewed.status,
        summary: "attached controller review and security-review evidence".to_string(),
    });
    emit_narrative(&event_writer, &event_builder, &RunId(state.goal_id.clone()), "review completed").await;
    let blocker = readiness_blocker(&state.goal_id, &reviewed).await?;
    if blocker.reason.contains("manual integration acceptance")
        && options.delivery_policy != GoalDeliveryPolicy::Local
    {
        return finalize_until_ready_delivery(
            &state.goal_id,
            steps,
            options.delivery_policy,
            options.merge_policy,
            project_dir,
        )
        .await;
    }
    finalize_until_ready_blocker(&state.goal_id, steps, blocker).await
}

fn verification_summary(proof: &GoalProof) -> String {
    if proof.gates.is_empty() {
        return "no verification gates were detected or configured".to_string();
    }
    let passed = proof.gates.iter().filter(|gate| gate.passed).count();
    format!(
        "ran {} verification gate(s), {passed} passed",
        proof.gates.len()
    )
}

fn verification_can_continue(proof: &GoalProof) -> bool {
    !proof.gates.is_empty() && crate::runtime::gates::gates_passed(&proof.gates)
}

fn proof_can_continue(proof: &GoalProof) -> bool {
    matches!(proof.status, GoalStatus::NotReady | GoalStatus::Running)
        && verification_can_continue(proof)
}

fn verification_blocker(proof: &GoalProof) -> String {
    if proof.gates.is_empty() {
        return "verification blocked: no local gates were detected or configured".to_string();
    }
    let failed = proof
        .gates
        .iter()
        .filter(|gate| !gate.passed)
        .map(|gate| gate.name.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    format!("verification blocked: required gate(s) failed: {failed}")
}

fn terminal_blocker(proof: &GoalProof) -> UntilReadyBlocker {
    match proof.status {
        GoalStatus::NeedsMoreBudget => {
            UntilReadyBlocker::policy("budget exhausted before proof-backed readiness")
        }
        GoalStatus::Paused => {
            UntilReadyBlocker::policy("goal paused before proof-backed readiness")
        }
        GoalStatus::Cancelled => {
            UntilReadyBlocker::policy("goal cancelled before proof-backed readiness")
        }
        GoalStatus::BlockedOnHuman => UntilReadyBlocker::human(
            proof
                .human_decisions_required
                .first()
                .cloned()
                .unwrap_or_else(|| {
                    "human decision required before execution can continue".to_string()
                }),
            UNTIL_READY_BLOCKER_FILE,
        ),
        GoalStatus::BlockedOnExternal => {
            UntilReadyBlocker::policy("external dependency blocked goal execution")
        }
        GoalStatus::FailedInfra => {
            UntilReadyBlocker::policy("infrastructure failure blocked goal execution")
        }
        _ => UntilReadyBlocker::policy(verification_blocker(proof)),
    }
}

async fn has_pending_agent_dispatch(goal_id: &str) -> Result<bool> {
    let state = crate::runtime::goal::resolve_goal(goal_id).await?;
    let task_graph = GoalTaskGraph::load(&state.state_dir).await?;
    if state.slice_execution {
        let done = all_slices_done(&state.state_dir, &task_graph).await?;
        return Ok(!done);
    }
    Ok(crate::runtime::goal::agent::goal_agent_dispatch_plan(&state, &task_graph).is_some())
}

async fn readiness_blocker(goal_id: &str, proof: &GoalProof) -> Result<UntilReadyBlocker> {
    let state = crate::runtime::goal::resolve_goal(goal_id).await?;
    let task_graph = GoalTaskGraph::load(&state.state_dir).await?;
    let blocked_tasks = task_graph
        .tasks
        .iter()
        .filter(|task| task.status == GoalTaskStatus::Blocked)
        .map(|task| task.id.as_str())
        .collect::<Vec<_>>();
    if !blocked_tasks.is_empty() {
        return Ok(UntilReadyBlocker::policy(format!(
            "blocked task(s): {}; inspect task-graph.json and proof.json for policy evidence",
            blocked_tasks.join(", ")
        )));
    }
    if let Some(blocker) = review_wall_blocker(proof) {
        return Ok(UntilReadyBlocker::policy(blocker));
    }
    if manual_integration_acceptance_required(&task_graph, proof) {
        return Ok(UntilReadyBlocker::human(
            "manual integration acceptance is required before ready; local delivery policy keeps GitHub mutation and merge disabled"
                .to_string(),
            MANUAL_INTEGRATION_BLOCKER_FILE,
        ));
    }
    Ok(UntilReadyBlocker::policy(
        proof
            .known_gaps
            .first()
            .cloned()
            .unwrap_or_else(|| "proof remains not_ready without a ready claim".to_string()),
    ))
}

fn review_wall_blocker(proof: &GoalProof) -> Option<String> {
    proof
        .known_gaps
        .iter()
        .find(|gap| gap.contains("review is blocked") || gap.contains("review artifact"))
        .cloned()
}

fn manual_integration_acceptance_required(task_graph: &GoalTaskGraph, proof: &GoalProof) -> bool {
    goal_task_done(task_graph, state::GOAL_LOCAL_VERIFY_TASK_ID)
        && crate::runtime::goal::task_graph::goal_agent_execution_done(task_graph)
        && goal_task_done(task_graph, state::GOAL_REVIEW_TASK_ID)
        && goal_task_done(task_graph, state::GOAL_SECURITY_REVIEW_TASK_ID)
        && !proof.changed_files.is_empty()
        && proof.post_mutation_gates_ran
}

async fn cleanup_goal_worktrees(state: &state::GoalState, project_dir: &Path) {
    if !state.slice_execution {
        return;
    }
    if let Ok(records) =
        crate::runtime::goal::task_graph::load_goal_task_delivery_records(&state.state_dir).await
    {
        let paths: Vec<PathBuf> = records
            .into_iter()
            .filter_map(|r| r.metadata.worktree_path)
            .collect();
        remove_goal_worktrees(project_dir, &paths).await;
    }
}

async fn finalize_until_ready_blocker(
    goal_id: &str,
    mut steps: Vec<GoalControllerStep>,
    blocker: UntilReadyBlocker,
) -> Result<GoalRunUntilReadyOutcome> {
    let mut state = crate::runtime::goal::resolve_goal(goal_id).await?;
    let mut proof = GoalProof::load(&state.state_dir).await?;
    let now = Utc::now();
    let relative_path = PathBuf::from(blocker.artifact_file);
    if let Some(parent) = relative_path.parent() {
        crate::runtime::config::ensure_private_dir(&state.state_dir.join(parent)).await?;
    }
    let artifact = json!({
        "status": "blocked",
        "reason": &blocker.reason,
        "delivery_policy": "local",
        "merge_policy": "manual",
        "github_mutation": false,
        "integrator_acceptance": "manual",
        "proof": state::GOAL_PROOF_FILE,
        "recorded_at": now,
    });
    proof::write_json_artifact(&state.state_dir.join(&relative_path), &artifact).await?;
    evidence::record_artifact_path_once(&mut state, "policy_blocker", relative_path.clone(), now);
    state.updated_at = now;
    FileSystemGoalStateStore::new().save(&state).await?;

    push_unique(&mut proof.known_gaps, blocker.reason.clone());
    if blocker.human_decision_required {
        push_unique(&mut proof.human_decisions_required, blocker.reason.clone());
    }
    proof.generated_at = now;
    proof.artifacts = state.artifacts.clone();
    proof::write_json_artifact(&state.state_dir.join(state::GOAL_PROOF_FILE), &proof).await?;

    steps.push(GoalControllerStep {
        kind: GoalControllerStepKind::Blocked,
        status: proof.status,
        summary: blocker.reason.clone(),
    });
    Ok(GoalRunUntilReadyOutcome {
        state,
        proof,
        steps,
        blocker: Some(blocker.reason),
        policy_evidence_path: Some(relative_path),
    })
}

async fn finalize_slice_integrator(
    goal_id: &str,
    mut steps: Vec<GoalControllerStep>,
    policy: GoalDeliveryPolicy,
    merge_policy: GoalMergePolicy,
    project_dir: &Path,
) -> Result<GoalRunUntilReadyOutcome> {
    let mut state = crate::runtime::goal::resolve_goal(goal_id).await?;
    let mut proof = GoalProof::load(&state.state_dir).await?;
    let now = Utc::now();

    // Collect delivered slice branches
    let records =
        crate::runtime::goal::task_graph::load_goal_task_delivery_records(&state.state_dir)
            .await?;
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

    // Create integrator branch from current master/main
    let base_branch = resolve_base_branch(project_dir).await.unwrap_or_else(|| "master".to_string());
    if let Err(e) = create_integrator_branch(project_dir, &integrator_branch, &base_branch).await {
        return finalize_until_ready_blocker(
            goal_id,
            steps,
            UntilReadyBlocker::policy(format!(
                "integrator branch creation failed: {e}"
            )),
        )
        .await;
    }

    // Merge each slice branch into the integrator
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

    // Push integrator branch
    if let Err(e) = push_branch(project_dir, &integrator_branch).await {
        return finalize_until_ready_blocker(
            goal_id,
            steps,
            UntilReadyBlocker::policy(format!("integrator push failed: {e}")),
        )
        .await;
    }

    // Run verification wall on the integrator branch
    let integrator_gate_config = crate::runtime::gates::load_or_detect_gates(project_dir).await;
    let integrator_gate_artifacts = state
        .state_dir
        .join(state::GOAL_ARTIFACTS_DIR)
        .join(state::GOAL_GATE_ARTIFACTS_DIR)
        .join("integrator");
    let integrator_gates = crate::runtime::gates::run_gates_with_evidence(
        &integrator_gate_config,
        project_dir,
        Some(&integrator_gate_artifacts),
    )
    .await;
    let _ = crate::runtime::goal::verifier::append_gate_events(&state, &integrator_gates).await;
    let integrator_gates_ok =
        !integrator_gates.is_empty() && crate::runtime::gates::gates_passed(&integrator_gates);
    if !integrator_gates_ok {
        let _ = git_command(
            project_dir,
            vec![
                std::ffi::OsString::from("checkout"),
                std::ffi::OsString::from(&base_branch),
            ],
        )
        .await;
        return finalize_until_ready_blocker(
            goal_id,
            steps,
            UntilReadyBlocker::policy(
                "integrator verification gates failed; switched back to base branch".to_string(),
            ),
        )
        .await;
    }
    steps.push(GoalControllerStep {
        kind: GoalControllerStepKind::Verify,
        status: GoalStatus::Ready,
        summary: format!(
            "integrator verification wall passed ({} gate(s))",
            integrator_gates.len()
        ),
    });
    let integrator_narrative = format!(
        "integrator branch passed {} verification gate(s)",
        integrator_gates.len()
    );
    let integrator_event_writer = EventWriter::new(
        state.state_dir.join(crate::runtime::config::EVENTS_FILE),
    );
    if let Ok(event) = Event::new(
        RunId(state.goal_id.clone()),
        EventKind::TaskOutput,
    )
    .with_actor("controller")
    .with_message(&integrator_narrative)
    {
        let _ = integrator_event_writer.append(&event).await;
    }

    // Open integrator PR
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

    let pr_request = crate::runtime::goal::delivery::GoalGithubPrRequest {
        title,
        body,
        head_branch: integrator_branch.clone(),
        base_branch: Some(base_branch),
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

    // Apply merge policy
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
            proof.git = super::super::evidence::detect_git_evidence(project_dir)
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
                "no integrator PR was created; inspect the goal state for delivery evidence".to_string()
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
            proof.git = super::super::evidence::detect_git_evidence(project_dir)
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

            Ok(GoalRunUntilReadyOutcome {
                state,
                proof,
                steps,
                blocker: Some(instruction),
                policy_evidence_path: Some(PathBuf::from(MANUAL_INTEGRATION_BLOCKER_FILE)),
            })
        }
        GoalMergePolicy::Gated => {
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
                client.merge_pr(url).await?;
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
            proof.git = super::super::evidence::detect_git_evidence(project_dir)
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

async fn resolve_base_branch(repo_dir: &Path) -> Option<String> {
    for branch in ["main", "master"] {
        let output = git_command(
            repo_dir,
            vec![
                std::ffi::OsString::from("show-ref"),
                std::ffi::OsString::from("--verify"),
                std::ffi::OsString::from("--quiet"),
                std::ffi::OsString::from(format!("refs/heads/{branch}")),
            ],
        )
        .await
        .ok()?;
        if output.status.success() {
            return Some(branch.to_string());
        }
    }
    None
}

async fn create_integrator_branch(
    repo_dir: &Path,
    integrator_branch: &str,
    base_branch: &str,
) -> anyhow::Result<()> {
    let output = git_command(
        repo_dir,
        vec![
            std::ffi::OsString::from("checkout"),
            std::ffi::OsString::from("-b"),
            std::ffi::OsString::from(integrator_branch),
            std::ffi::OsString::from(base_branch),
        ],
    )
    .await?;
    if output.status.success() {
        Ok(())
    } else {
        anyhow::bail!("git checkout -b failed: {}", output_stderr(&output))
    }
}

async fn merge_branch_into_integrator(
    repo_dir: &Path,
    branch: &str,
    integrator_branch: &str,
) -> anyhow::Result<()> {
    // Ensure we're on the integrator branch
    let checkout = git_command(
        repo_dir,
        vec![
            std::ffi::OsString::from("checkout"),
            std::ffi::OsString::from(integrator_branch),
        ],
    )
    .await?;
    if !checkout.status.success() {
        anyhow::bail!("git checkout integrator failed: {}", output_stderr(&checkout));
    }

    let output = git_command(
        repo_dir,
        vec![
            std::ffi::OsString::from("merge"),
            std::ffi::OsString::from(branch),
            std::ffi::OsString::from("--no-edit"),
        ],
    )
    .await?;
    if output.status.success() {
        Ok(())
    } else {
        anyhow::bail!("git merge failed: {}", output_stderr(&output))
    }
}

async fn push_branch(repo_dir: &Path, branch: &str) -> anyhow::Result<()> {
    let output = git_command(
        repo_dir,
        vec![
            std::ffi::OsString::from("push"),
            std::ffi::OsString::from("-u"),
            std::ffi::OsString::from("origin"),
            std::ffi::OsString::from(branch),
        ],
    )
    .await?;
    if output.status.success() {
        Ok(())
    } else {
        anyhow::bail!("git push failed: {}", output_stderr(&output))
    }
}

async fn git_command(repo_dir: &Path, args: Vec<std::ffi::OsString>) -> anyhow::Result<std::process::Output> {
    let mut command = tokio::process::Command::new("git");
    command.arg("-C").arg(repo_dir).args(args);
    tokio::time::timeout(GIT_COMMAND_TIMEOUT, command.output())
        .await
        .with_context(|| "Timed out while running git command")?
        .with_context(|| "Failed to run git command")
}

fn output_stderr(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stderr).trim().to_string()
}

async fn finalize_until_ready_delivery(
    goal_id: &str,
    mut steps: Vec<GoalControllerStep>,
    policy: GoalDeliveryPolicy,
    merge_policy: GoalMergePolicy,
    project_dir: &Path,
) -> Result<GoalRunUntilReadyOutcome> {
    let state = crate::runtime::goal::resolve_goal(goal_id).await?;

    if state.slice_execution && policy != GoalDeliveryPolicy::Local {
        return finalize_slice_integrator(goal_id, steps, policy, merge_policy, project_dir).await;
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
            proof.git = super::super::evidence::detect_git_evidence(project_dir)
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
            proof.git = super::super::evidence::detect_git_evidence(project_dir)
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
                client.merge_pr(url).await?;
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
            proof.git = super::super::evidence::detect_git_evidence(project_dir)
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

fn push_unique(values: &mut Vec<String>, value: String) {
    if !values.iter().any(|existing| existing == &value) {
        values.push(value);
    }
}

async fn emit_narrative(
    writer: &EventWriter,
    _builder: &EventBuilder,
    run_id: &RunId,
    message: &str,
) {
    if let Ok(event) = Event::new(run_id.clone(), EventKind::TaskOutput)
        .with_actor("controller")
        .with_message(message)
    {
        let _ = writer.append(&event).await;
    }
}
