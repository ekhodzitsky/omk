use std::path::{Path, PathBuf};

use crate::runtime::goal::state::{GoalState, GoalStatus};
use crate::runtime::goal::task_graph::{GoalDeliverySlice, GoalTaskGraph, GoalTaskStatus};
use crate::runtime::goal::{evidence, proof, review, state, task_graph};

pub(crate) async fn process_slice_delivery_and_review(
    state: &GoalState,
    task_graph: &mut GoalTaskGraph,
    slice: &GoalDeliverySlice,
    agent_execution_succeeded: bool,
    exec_project_dir: &Path,
) -> anyhow::Result<()> {
    if agent_execution_succeeded
        && state.delivery_policy != crate::runtime::goal::GoalDeliveryPolicy::Local
    {
        let base_branch = crate::runtime::goal::control::resolve_base_branch(exec_project_dir).await;
        let delivery_options = crate::runtime::goal::delivery::SlicePrDeliveryOptions {
            policy: state.delivery_policy,
            dry_run: false,
            base_branch,
        };
        let delivery = crate::runtime::goal::delivery::deliver_slice_pr(
            exec_project_dir,
            slice,
            state,
            task_graph,
            delivery_options,
        )
        .await;

        let mut extra = serde_json::Map::new();
        let slice_status = match delivery {
            Ok(ref d) if d.pr_url.is_some() => task_graph::GoalTaskDeliveryStatus::Delivered,
            Ok(ref d) => {
                // Blocked by review wall or merge check inside deliver_slice_pr.
                if let Some(ref artifacts) = d.review_artifacts {
                    let anti_slop_confidence =
                        review::anti_slop_confidence_with_findings(artifacts, &d.slop_findings);
                    if anti_slop_confidence > review::ANTI_SLOP_ACTIONABLE_THRESHOLD {
                        let changed_files =
                            crate::runtime::gates::detect_changed_files(exec_project_dir).await;
                        let feedback_summary = artifacts
                            .iter()
                            .filter(|a| !a.passed)
                            .map(|a| format!("{}: {}", a.kind, a.feedback))
                            .collect::<Vec<_>>()
                            .join("; ");

                        if let Some(cleanup_task_id) = task_graph::spawn_cleanup_task(
                            task_graph,
                            &slice.task_id,
                            &feedback_summary,
                            &changed_files,
                            chrono::Utc::now(),
                        ) {
                            let writer = crate::runtime::events::EventWriter::new(
                                state.state_dir.join(crate::runtime::config::EVENTS_FILE),
                            );
                            let event = crate::runtime::events::Event::new(
                                crate::runtime::events::RunId(state.goal_id.clone()),
                                crate::runtime::events::EventKind::TaskGraphMutated,
                            )
                            .with_actor(state::GOAL_CONTROLLER_ACTOR)
                            .with_payload(
                                crate::runtime::events::TaskGraphMutationPayload {
                                    action: "task_added".to_string(),
                                    source: "anti_slop_cleanup".to_string(),
                                    task_id: crate::runtime::events::TaskId(cleanup_task_id),
                                    task_graph_path: PathBuf::from(state::GOAL_TASK_GRAPH_FILE),
                                    proposal_path: PathBuf::new(),
                                    total_tasks_after: task_graph.tasks.len(),
                                },
                            )?;
                            writer.append(&event).await?;
                        }

                        extra.insert(
                            "review_feedback".to_string(),
                            serde_json::Value::String(format!(
                                "Anti-slop confidence {anti_slop_confidence:.2} exceeds threshold. Cleanup task spawned for slice {}.",
                                slice.task_id
                            )),
                        );
                    } else {
                        extra.insert(
                            "review_feedback".to_string(),
                            serde_json::Value::String(d.reason.clone()),
                        );
                        if let Some(task) =
                            task_graph.tasks.iter_mut().find(|t| t.id == slice.task_id)
                        {
                            task.status = GoalTaskStatus::Pending;
                            task.completed_at = None;
                            task.description =
                                format!("{}\n\n[review-feedback] {}", task.description, d.reason);
                        }
                    }
                } else {
                    extra.insert(
                        "review_feedback".to_string(),
                        serde_json::Value::String(d.reason.clone()),
                    );
                    if let Some(task) = task_graph.tasks.iter_mut().find(|t| t.id == slice.task_id)
                    {
                        task.status = GoalTaskStatus::Pending;
                        task.completed_at = None;
                        task.description =
                            format!("{}\n\n[review-feedback] {}", task.description, d.reason);
                    }
                }
                task_graph::GoalTaskDeliveryStatus::Blocked
            }
            Err(ref e) => {
                let error_msg = format!("Slice delivery error: {e}");
                extra.insert(
                    "review_feedback".to_string(),
                    serde_json::Value::String(error_msg.clone()),
                );
                if let Some(task) = task_graph.tasks.iter_mut().find(|t| t.id == slice.task_id) {
                    task.status = GoalTaskStatus::Pending;
                    task.completed_at = None;
                    task.description =
                        format!("{}\n\n[review-feedback] {}", task.description, error_msg);
                }
                task_graph::GoalTaskDeliveryStatus::Blocked
            }
        };

        task_graph::update_goal_task_delivery_metadata(
            &state.state_dir,
            &slice.task_id,
            task_graph::GoalTaskDeliveryMetadataUpdate {
                status: Some(slice_status),
                pr_url: delivery.as_ref().ok().and_then(|d| d.pr_url.clone()),
                commit_sha: delivery.as_ref().ok().and_then(|d| d.commit_sha.clone()),
                extra,
                ..Default::default()
            },
        )
        .await?;
    } else {
        let slice_status = if agent_execution_succeeded {
            task_graph::GoalTaskDeliveryStatus::Delivered
        } else {
            task_graph::GoalTaskDeliveryStatus::Blocked
        };
        task_graph::update_goal_task_delivery_metadata(
            &state.state_dir,
            &slice.task_id,
            task_graph::GoalTaskDeliveryMetadataUpdate {
                status: Some(slice_status),
                ..Default::default()
            },
        )
        .await?;
    }
    Ok(())
}

/// Merge gate results from multiple worktrees. A gate passes only if it passes
/// in ALL result sets where it appears.
pub(crate) fn merge_gate_results(
    base: &[crate::runtime::gates::GateResult],
    extra: &[crate::runtime::gates::GateResult],
) -> Vec<crate::runtime::gates::GateResult> {
    use std::collections::HashMap;

    let mut merged: HashMap<String, crate::runtime::gates::GateResult> =
        base.iter().map(|g| (g.name.clone(), g.clone())).collect();
    for g in extra {
        merged
            .entry(g.name.clone())
            .and_modify(|existing| {
                existing.passed &= g.passed;
                if !g.passed {
                    existing.stderr = g.stderr.to_string();
                }
            })
            .or_insert_with(|| g.clone());
    }
    merged.into_values().collect()
}

/// Aggregate evidence from multiple concurrent slices into a single
/// `GoalAgentRunEvidence` representing the whole swarm.
pub(crate) fn aggregate_agent_evidence(
    slices: &[&evidence::GoalAgentRunEvidence],
    goal_id: &str,
) -> evidence::GoalAgentRunEvidence {
    use crate::runtime::scheduler::runner::RunSummary;

    let mut combined = evidence::GoalAgentRunEvidence {
        summary: RunSummary {
            run_id: format!("{goal_id}-concurrent"),
            completed: slices.iter().map(|e| e.summary.completed).sum(),
            failed: slices.iter().map(|e| e.summary.failed).sum(),
            cancelled: slices.iter().map(|e| e.summary.cancelled).sum(),
            total: slices.iter().map(|e| e.summary.total).sum(),
        },
        run_path: PathBuf::new(),
        task_policy_path: PathBuf::new(),
        agent_task_proposals_path: PathBuf::new(),
        worker_outbox_path: PathBuf::new(),
        wire_events_path: PathBuf::new(),
        mutation_diff_path: PathBuf::new(),
        changed_files_path: PathBuf::new(),
        changed_files: Vec::new(),
        accepted_task_count: 0,
        rejected_task_count: 0,
        accepted_task_ids: Vec::new(),
        agent_proposed_tasks: Vec::new(),
        worker_results: Vec::new(),
        worker_summary: None,
    };

    for ev in slices {
        combined
            .changed_files
            .extend(ev.changed_files.iter().cloned());
        combined.accepted_task_count += ev.accepted_task_count;
        combined.rejected_task_count += ev.rejected_task_count;
        combined
            .accepted_task_ids
            .extend(ev.accepted_task_ids.iter().cloned());
        combined
            .agent_proposed_tasks
            .extend(ev.agent_proposed_tasks.iter().cloned());
        combined
            .worker_results
            .extend(ev.worker_results.iter().cloned());
        if combined.worker_summary.is_none() && ev.worker_summary.is_some() {
            combined.worker_summary = ev.worker_summary.clone();
        }
        if combined.run_path.as_os_str().is_empty() && !ev.run_path.as_os_str().is_empty() {
            combined.run_path = ev.run_path.clone();
        }
        if combined.task_policy_path.as_os_str().is_empty()
            && !ev.task_policy_path.as_os_str().is_empty()
        {
            combined.task_policy_path = ev.task_policy_path.clone();
        }
        if combined.agent_task_proposals_path.as_os_str().is_empty()
            && !ev.agent_task_proposals_path.as_os_str().is_empty()
        {
            combined.agent_task_proposals_path = ev.agent_task_proposals_path.clone();
        }
        if combined.worker_outbox_path.as_os_str().is_empty()
            && !ev.worker_outbox_path.as_os_str().is_empty()
        {
            combined.worker_outbox_path = ev.worker_outbox_path.clone();
        }
        if combined.wire_events_path.as_os_str().is_empty()
            && !ev.wire_events_path.as_os_str().is_empty()
        {
            combined.wire_events_path = ev.wire_events_path.clone();
        }
        if combined.mutation_diff_path.as_os_str().is_empty()
            && !ev.mutation_diff_path.as_os_str().is_empty()
        {
            combined.mutation_diff_path = ev.mutation_diff_path.clone();
        }
        if combined.changed_files_path.as_os_str().is_empty()
            && !ev.changed_files_path.as_os_str().is_empty()
        {
            combined.changed_files_path = ev.changed_files_path.clone();
        }
    }

    combined.changed_files.sort();
    combined.changed_files.dedup();
    combined
}

pub(crate) fn ensure_goal_can_continue(state: &crate::runtime::goal::state::GoalState) -> anyhow::Result<()> {
    if state.status == GoalStatus::Paused {
        anyhow::bail!(
            "Goal '{}' is paused; run `omk goal resume {}` before continuing",
            state.goal_id,
            state.goal_id
        );
    }
    if state.status == GoalStatus::BlockedOnHuman {
        let reason = state
            .failure
            .as_ref()
            .map(|failure| failure.reason.as_str())
            .unwrap_or("human decision required");
        anyhow::bail!("Goal '{}' is blocked_on_human: {reason}", state.goal_id);
    }
    Ok(())
}

pub(crate) async fn append_proof_event(
    state: &GoalState,
    proof: &proof::GoalProof,
) -> anyhow::Result<()> {
    let writer = crate::runtime::events::EventWriter::new(
        state.state_dir.join(crate::runtime::config::EVENTS_FILE),
    );
    let builder = crate::runtime::events::EventBuilder::new(crate::runtime::events::RunId(
        state.goal_id.clone(),
    ));
    writer
        .append(&builder.proof_written(
            &state.state_dir.join(state::GOAL_PROOF_FILE),
            &proof.status.to_string(),
        )?)
        .await
}
