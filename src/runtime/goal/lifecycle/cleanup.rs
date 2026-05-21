use std::path::{Path, PathBuf};

use crate::runtime::goal::state::{GoalState, GoalStatus};
use crate::runtime::goal::task_graph::{GoalDeliverySlice, GoalTask, GoalTaskEvidence, GoalTaskGraph, GoalTaskStatus};
use crate::runtime::goal::verifier::{
    scan_goal_security_findings_structured, SecurityFinding,
};
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
        let base_branch =
            crate::runtime::goal::control::resolve_base_branch(exec_project_dir).await;
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

        // Spawn a refactor follow-up task whenever anti-slop evidence shows
        // concrete rough edges, regardless of whether delivery succeeded.
        let slop_findings: Vec<_> = delivery
            .as_ref()
            .map(|d| d.slop_findings.clone())
            .unwrap_or_default();
        if !slop_findings.is_empty() {
            let changed_files = crate::runtime::gates::detect_changed_files(exec_project_dir).await;
            if let Some(refactor_task_id) = task_graph::spawn_refactor_task_from_slop_findings(
                task_graph,
                &slice.task_id,
                &slop_findings,
                &changed_files,
                chrono::Utc::now(),
            ) {
                // Best-effort event emission: the task graph has already been
                // mutated, so we must not fail the whole delivery if the event
                // writer encounters a transient I/O error.
                let writer = crate::runtime::events::EventWriter::new(
                    state.state_dir.join(crate::runtime::config::EVENTS_FILE),
                );
                if let Ok(event) = crate::runtime::events::Event::new(
                    crate::runtime::events::RunId(state.goal_id.clone()),
                    crate::runtime::events::EventKind::TaskGraphMutated,
                )
                .with_actor(state::GOAL_CONTROLLER_ACTOR)
                .with_payload(crate::runtime::events::TaskGraphMutationPayload {
                    action: "task_added".to_string(),
                    source: "anti_slop_refactor".to_string(),
                    task_id: crate::runtime::events::TaskId(refactor_task_id),
                    task_graph_path: PathBuf::from(state::GOAL_TASK_GRAPH_FILE),
                    proposal_path: PathBuf::new(),
                    total_tasks_after: task_graph.tasks.len(),
                }) {
                    let _ = writer.append(&event).await;
                }
            }

            extra.insert(
                "review_feedback".to_string(),
                serde_json::Value::String(format!(
                    "Anti-slop review found {} rough edge(s). Refactor task spawned for slice {}.",
                    slop_findings.len(),
                    slice.task_id
                )),
            );
        }

        // Spawn a security cleanup task whenever security findings are present.
        let changed_files = crate::runtime::gates::detect_changed_files(exec_project_dir).await;
        let security_findings =
            scan_goal_security_findings_structured(exec_project_dir, &changed_files).await?;
        if !security_findings.is_empty() {
            if let Some(security_task_id) = spawn_security_cleanup_task_from_findings(
                task_graph,
                &slice.task_id,
                &security_findings,
                &changed_files,
                chrono::Utc::now(),
            ) {
                let writer = crate::runtime::events::EventWriter::new(
                    state.state_dir.join(crate::runtime::config::EVENTS_FILE),
                );
                if let Ok(event) = crate::runtime::events::Event::new(
                    crate::runtime::events::RunId(state.goal_id.clone()),
                    crate::runtime::events::EventKind::TaskGraphMutated,
                )
                .with_actor(state::GOAL_CONTROLLER_ACTOR)
                .with_payload(crate::runtime::events::TaskGraphMutationPayload {
                    action: "task_added".to_string(),
                    source: "security_cleanup".to_string(),
                    task_id: crate::runtime::events::TaskId(security_task_id),
                    task_graph_path: PathBuf::from(state::GOAL_TASK_GRAPH_FILE),
                    proposal_path: PathBuf::new(),
                    total_tasks_after: task_graph.tasks.len(),
                }) {
                    let _ = writer.append(&event).await;
                }
            }

            extra.insert(
                "review_feedback".to_string(),
                serde_json::Value::String(format!(
                    "Security review found {} high-confidence secret marker(s). Security cleanup task spawned for slice {}.",
                    security_findings.len(),
                    slice.task_id
                )),
            );
        }

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

pub(crate) fn ensure_goal_can_continue(
    state: &crate::runtime::goal::state::GoalState,
) -> anyhow::Result<()> {
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

pub(crate) fn spawn_security_cleanup_task_from_findings(
    task_graph: &mut GoalTaskGraph,
    slice_task_id: &str,
    findings: &[SecurityFinding],
    _changed_files: &[String],
    generated_at: chrono::DateTime<chrono::Utc>,
) -> Option<String> {
    let actionable: Vec<&SecurityFinding> = findings
        .iter()
        .filter(|f| !f.kind.is_quarantine_only())
        .collect();
    if actionable.is_empty() {
        return None;
    }

    let kind = "security-cleanup";
    let task_id = format!("goal-agent-{kind}-{slice_task_id}");
    let description = build_security_cleanup_description(&actionable);

    // Upsert pattern: update existing task if description changed, otherwise
    // create new.
    if let Some(existing) = task_graph
        .tasks
        .iter_mut()
        .find(|t| t.id == task_id)
    {
        if existing.description != description {
            existing.description = description;
            existing.status = GoalTaskStatus::Pending;
            existing.completed_at = None;
            existing.retry_count = 0;
        }
        return Some(task_id);
    }

    let new_task = GoalTask {
        id: task_id.clone(),
        title: "Security cleanup".to_string(),
        description,
        status: GoalTaskStatus::Pending,
        owner_role: Some(kind.to_string()),
        completed_at: None,
        evidence: vec![GoalTaskEvidence {
            kind: "security_verifier".to_string(),
            path: PathBuf::new(),
            summary: format!(
                "kind={} generated_at={} findings_count={}",
                kind,
                generated_at.to_rfc3339(),
                findings.len()
            ),
        }],
        retry_count: 0,
        max_retries: 3,
        lease_expires_at: None,
        dependencies: vec![slice_task_id.to_string()],
        read_set: actionable.iter().map(|f| f.path.clone()).collect(),
        write_set: actionable.iter().map(|f| f.path.clone()).collect(),
        risk: "critical".to_string(),
        acceptance: vec![
            "All secret markers are removed or rotated".to_string(),
            "No raw credentials remain in source files".to_string(),
        ],
    };

    // Make the slice task depend on the cleanup task so it cannot be
    // re-delivered until cleanup is done.
    if let Some(slice_task) = task_graph
        .tasks
        .iter_mut()
        .find(|t| t.id == slice_task_id)
    {
        slice_task
            .dependencies
            .push(task_id.clone());
        slice_task.status = GoalTaskStatus::Pending;
    }

    task_graph.tasks.push(new_task);
    Some(task_id)
}

fn build_security_cleanup_description(findings: &[&SecurityFinding]) -> String {
    use crate::wire::protocol::redact_wire_secrets;
    let mut lines =
        vec!["Auto-security-cleanup task generated from security verifier findings:".to_string()];
    for finding in findings {
        let location = finding
            .line
            .map(|l| format!("line {}", l))
            .unwrap_or_else(|| "file level".to_string());
        let redacted = finding
            .evidence_snippet
            .as_ref()
            .map(|s| redact_wire_secrets(&serde_json::json!(s)).to_string())
            .unwrap_or_default();
        lines.push(format!(
            "- [{}] {} at {}: {}",
            finding.kind, finding.path, location, redacted
        ));
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::goal::verifier::SecurityFindingKind;
    use chrono::Utc;

    fn slice_task(id: &str) -> GoalTask {
        GoalTask {
            id: id.to_string(),
            title: format!("Task {id}"),
            description: format!("Task {id} description"),
            status: GoalTaskStatus::Done,
            owner_role: None,
            completed_at: None,
            evidence: Vec::new(),
            retry_count: 0,
            max_retries: 0,
            lease_expires_at: None,
            dependencies: Vec::new(),
            read_set: Vec::new(),
            write_set: Vec::new(),
            risk: "low".to_string(),
            acceptance: vec![format!("Task {id} acceptance")],
        }
    }

    fn graph(tasks: Vec<GoalTask>) -> GoalTaskGraph {
        GoalTaskGraph {
            version: 1,
            goal_id: "goal-test".to_string(),
            generated_at: Utc::now(),
            tasks,
        }
    }

    #[test]
    fn spawn_security_cleanup_returns_none_on_empty_findings() {
        let mut tg = graph(vec![slice_task("slice-a")]);
        let result = spawn_security_cleanup_task_from_findings(
            &mut tg,
            "slice-a",
            &[],
            &["src/main.rs".to_string()],
            Utc::now(),
        );
        assert!(result.is_none());
        assert_eq!(tg.tasks.len(), 1);
    }

    #[test]
    fn spawn_security_cleanup_creates_task_for_private_key_finding() {
        let mut tg = graph(vec![slice_task("slice-a")]);
        let findings = vec![SecurityFinding {
            path: "src/secrets.txt".to_string(),
            kind: SecurityFindingKind::PrivateKey,
            line: Some(5),
            evidence_snippet: Some("-----BEGIN PRIVATE KEY-----".to_string()),
        }];
        let result = spawn_security_cleanup_task_from_findings(
            &mut tg,
            "slice-a",
            &findings,
            &["src/secrets.txt".to_string()],
            Utc::now(),
        );
        assert_eq!(
            result,
            Some("goal-agent-security-cleanup-slice-a".to_string())
        );
        assert_eq!(tg.tasks.len(), 2);

        let cleanup = tg
            .tasks
            .iter()
            .find(|t| t.id == "goal-agent-security-cleanup-slice-a")
            .expect("security cleanup task exists");
        assert!(cleanup.description.contains("private_key"));
        assert!(cleanup.description.contains("src/secrets.txt"));
        assert!(cleanup.description.contains("line 5"));
        assert_eq!(cleanup.status, GoalTaskStatus::Pending);
        assert_eq!(cleanup.owner_role, Some("security-cleanup".to_string()));

        let slice = tg
            .tasks
            .iter()
            .find(|t| t.id == "slice-a")
            .expect("slice task exists");
        assert!(slice
            .dependencies
            .contains(&"goal-agent-security-cleanup-slice-a".to_string()));
        assert_eq!(slice.status, GoalTaskStatus::Pending);
    }

    #[test]
    fn spawn_security_cleanup_does_not_leak_secret_value_into_task_prompt() {
        let mut tg = graph(vec![slice_task("slice-a")]);
        let secret_value = "ghp_abcdefghijklmnopqrstuvwxyz123";
        let findings = vec![SecurityFinding {
            path: "src/config.rs".to_string(),
            kind: SecurityFindingKind::SecretAssignment,
            line: Some(10),
            evidence_snippet: Some(format!("api_key = \"{secret_value}\"")),
        }];
        let result = spawn_security_cleanup_task_from_findings(
            &mut tg,
            "slice-a",
            &findings,
            &["src/config.rs".to_string()],
            Utc::now(),
        );
        assert!(result.is_some());

        let cleanup = tg
            .tasks
            .iter()
            .find(|t| t.id == "goal-agent-security-cleanup-slice-a")
            .unwrap();
        assert!(!cleanup.description.contains(secret_value));
        assert!(cleanup.description.contains("[REDACTED]"));
    }

    #[test]
    fn spawn_security_cleanup_skips_quarantine_only_findings() {
        let mut tg = graph(vec![slice_task("slice-a")]);
        let findings = vec![SecurityFinding {
            path: "src/huge.log".to_string(),
            kind: SecurityFindingKind::OversizedFile,
            line: None,
            evidence_snippet: None,
        }];
        let result = spawn_security_cleanup_task_from_findings(
            &mut tg,
            "slice-a",
            &findings,
            &["src/huge.log".to_string()],
            Utc::now(),
        );
        assert!(result.is_none());
        assert_eq!(tg.tasks.len(), 1);
    }
}
