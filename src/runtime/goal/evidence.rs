use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use super::proof::write_json_artifact;
use super::state::{
    GoalArtifact, GoalState, GOAL_ARTIFACTS_DIR, GOAL_GATE_ARTIFACTS_DIR, GOAL_PROOF_FILE,
};
use super::task_graph::GoalTaskEvidence;
use crate::git::GitRepo;
use crate::runtime::gates::detect_changed_files;
use crate::runtime::scheduler::runner::RunSummary;
use crate::runtime::worker::WorkerResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalGitEvidence {
    pub branch: String,
    pub head: String,
    pub dirty: bool,
}

#[derive(Debug, Clone)]
pub struct GoalAgentRunEvidence {
    pub summary: RunSummary,
    pub run_path: PathBuf,
    pub task_policy_path: PathBuf,
    pub agent_task_proposals_path: PathBuf,
    pub worker_outbox_path: PathBuf,
    pub wire_events_path: PathBuf,
    pub mutation_diff_path: PathBuf,
    pub changed_files_path: PathBuf,
    pub changed_files: Vec<String>,
    pub accepted_task_count: usize,
    pub rejected_task_count: usize,
    pub accepted_task_ids: Vec<String>,
    pub agent_proposed_tasks: Vec<super::agent::GoalAgentTaskProposal>,
    pub worker_results: Vec<WorkerResult>,
    pub worker_summary: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct GoalReviewEvidence {
    pub(crate) review_path: PathBuf,
    pub(crate) security_review_path: PathBuf,
    pub(crate) review_summary: String,
    pub(crate) security_summary: String,
    pub(crate) security_findings: Vec<String>,
}

pub(crate) fn record_artifact(
    state: &mut GoalState,
    kind: &str,
    path: &str,
    created_at: DateTime<Utc>,
) {
    state.artifacts.push(GoalArtifact {
        kind: kind.to_string(),
        path: PathBuf::from(path),
        created_at,
    });
}

pub(crate) fn record_artifact_path_once(
    state: &mut GoalState,
    kind: &str,
    path: PathBuf,
    created_at: DateTime<Utc>,
) {
    if state
        .artifacts
        .iter()
        .any(|artifact| artifact.kind == kind && artifact.path == path)
    {
        return;
    }
    state.artifacts.push(GoalArtifact {
        kind: kind.to_string(),
        path,
        created_at,
    });
}

pub(crate) async fn detect_git_evidence(project_dir: &Path) -> Option<GoalGitEvidence> {
    let repo = GitRepo::open(project_dir).ok()?;
    let branch = repo.current_branch().await.ok()?;
    let head = repo.head_commit_full().await.ok()?;
    let files = repo.changed_files().await.ok()?;

    Some(GoalGitEvidence {
        branch,
        head,
        dirty: !files.is_empty(),
    })
}

pub(crate) fn agent_execution_task_evidence(
    evidence: &GoalAgentRunEvidence,
    success: bool,
) -> Vec<GoalTaskEvidence> {
    let status = if success { "completed" } else { "blocked" };
    let worker_summary = evidence
        .worker_summary
        .as_deref()
        .filter(|summary| !summary.trim().is_empty())
        .unwrap_or("no worker summary recorded");
    let run_summary = format!(
        "Agent execution {status}: {}/{} scheduler task(s) completed; failed={}, cancelled={}. Worker summary: {worker_summary}",
        evidence.summary.completed,
        evidence.summary.total,
        evidence.summary.failed,
        evidence.summary.cancelled
    );
    let mutation_summary = if evidence.changed_files.is_empty() {
        "No project file changes were detected after the agent wave.".to_string()
    } else {
        format!(
            "Project mutation evidence captured for changed file(s): {}",
            evidence.changed_files.join(", ")
        )
    };

    let mut task_evidence = vec![
        GoalTaskEvidence {
            kind: "agent_run".to_string(),
            path: evidence.run_path.clone(),
            summary: run_summary,
        },
        GoalTaskEvidence {
            kind: "task_policy".to_string(),
            path: evidence.task_policy_path.clone(),
            summary: format!(
                "Controller task policy recorded: accepted={}, rejected={}, accepted_task_ids={}",
                evidence.accepted_task_count,
                evidence.rejected_task_count,
                evidence.accepted_task_ids.join(", ")
            ),
        },
    ];
    if !evidence.agent_proposed_tasks.is_empty() {
        task_evidence.push(GoalTaskEvidence {
            kind: "agent_task_proposals".to_string(),
            path: evidence.agent_task_proposals_path.clone(),
            summary: format!(
                "Agent proposed {} follow-up task(s) for controller validation.",
                evidence.agent_proposed_tasks.len()
            ),
        });
    }
    task_evidence.extend([
        GoalTaskEvidence {
            kind: "worker_outbox".to_string(),
            path: evidence.worker_outbox_path.clone(),
            summary: "Worker outbox records the scheduler-visible task result.".to_string(),
        },
        GoalTaskEvidence {
            kind: "wire_events".to_string(),
            path: evidence.wire_events_path.clone(),
            summary: "Wire event stream records the agent protocol turn.".to_string(),
        },
        GoalTaskEvidence {
            kind: "mutation_diff".to_string(),
            path: evidence.mutation_diff_path.clone(),
            summary: mutation_summary,
        },
        GoalTaskEvidence {
            kind: "changed_files".to_string(),
            path: evidence.changed_files_path.clone(),
            summary: "Changed-file snapshot captured after the agent wave.".to_string(),
        },
    ]);
    task_evidence
}

pub(crate) fn agent_followup_task_evidence(
    evidence: &GoalAgentRunEvidence,
    result: Option<&WorkerResult>,
    success: bool,
) -> Vec<GoalTaskEvidence> {
    let status = if success { "completed" } else { "blocked" };
    let summary = result
        .map(|result| result.summary.clone())
        .or_else(|| evidence.worker_summary.clone())
        .unwrap_or_else(|| "No worker result was recorded for this follow-up task.".to_string());

    vec![
        GoalTaskEvidence {
            kind: "agent_run".to_string(),
            path: evidence.run_path.clone(),
            summary: format!(
                "Agent follow-up task {status} via run {}: {summary}",
                evidence.summary.run_id
            ),
        },
        GoalTaskEvidence {
            kind: "worker_outbox".to_string(),
            path: evidence.worker_outbox_path.clone(),
            summary: "Worker result evidence for this follow-up task.".to_string(),
        },
        GoalTaskEvidence {
            kind: "wire_events".to_string(),
            path: evidence.wire_events_path.clone(),
            summary: "Wire event evidence captured during follow-up execution.".to_string(),
        },
        GoalTaskEvidence {
            kind: "mutation_diff".to_string(),
            path: evidence.mutation_diff_path.clone(),
            summary: "Mutation diff captured after the follow-up wave.".to_string(),
        },
        GoalTaskEvidence {
            kind: "changed_files".to_string(),
            path: evidence.changed_files_path.clone(),
            summary: "Changed-file snapshot captured after the follow-up wave.".to_string(),
        },
    ]
}

pub(crate) fn local_verification_task_evidence(
    gates: &[crate::runtime::gates::GateResult],
    gates_ok: bool,
) -> Vec<GoalTaskEvidence> {
    let passed = gates.iter().filter(|gate| gate.passed).count();
    let gate_summary = if gates_ok {
        format!(
            "Local verification passed: {passed}/{} gate(s) succeeded.",
            gates.len()
        )
    } else if gates.is_empty() {
        "Local verification found no configured gates.".to_string()
    } else {
        format!(
            "Local verification is blocked: {passed}/{} gate(s) succeeded.",
            gates.len()
        )
    };

    vec![
        GoalTaskEvidence {
            kind: "gate_artifacts".to_string(),
            path: PathBuf::from(GOAL_ARTIFACTS_DIR).join(GOAL_GATE_ARTIFACTS_DIR),
            summary: gate_summary,
        },
        GoalTaskEvidence {
            kind: "proof".to_string(),
            path: PathBuf::from(GOAL_PROOF_FILE),
            summary: "Goal proof refreshed from local verification evidence.".to_string(),
        },
    ]
}

pub(crate) async fn write_goal_agent_mutation_snapshot(
    state: &GoalState,
    project_dir: &Path,
    mutation_diff_path: &Path,
    changed_files_path: &Path,
) -> Result<Vec<String>> {
    let changed_files = detect_changed_files(project_dir).await;
    let diff = git_diff(project_dir).await.unwrap_or_default();
    let body = if diff.trim().is_empty() {
        if changed_files.is_empty() {
            "No project file changes were detected after the agent wave.\n".to_string()
        } else {
            format!(
                "No tracked git diff was available. Changed files:\n{}\n",
                changed_files
                    .iter()
                    .map(|file| format!("- {file}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        }
    } else {
        diff
    };

    crate::runtime::atomic::atomic_write(
        &state.state_dir.join(mutation_diff_path),
        body.as_bytes(),
    )
    .await?;
    write_json_artifact(&state.state_dir.join(changed_files_path), &changed_files).await?;
    Ok(changed_files)
}

async fn git_diff(project_dir: &Path) -> Option<String> {
    let repo = GitRepo::open(project_dir).ok()?;
    repo.diff().await.ok()
}

pub(crate) fn extract_goal_agent_task_proposals(
    results: &[WorkerResult],
) -> Vec<super::agent::GoalAgentTaskProposal> {
    results
        .iter()
        .flat_map(|result| extract_goal_agent_task_proposals_from_text(&result.summary))
        .collect()
}

fn extract_goal_agent_task_proposals_from_text(
    summary: &str,
) -> Vec<super::agent::GoalAgentTaskProposal> {
    let mut proposals = Vec::new();
    let mut search = summary;
    while let Some(marker_pos) = search.find(super::state::GOAL_AGENT_TASK_PROPOSAL_MARKER) {
        let after_marker =
            &search[marker_pos + super::state::GOAL_AGENT_TASK_PROPOSAL_MARKER.len()..];
        let Some((json, consumed)) = extract_first_json_object(after_marker) else {
            break;
        };
        match serde_json::from_str::<super::agent::GoalAgentTaskProposal>(&json) {
            Ok(proposal) => proposals.push(proposal),
            Err(error) => tracing::warn!(
                error = %error,
                "Ignoring malformed agent task proposal"
            ),
        }
        search = &after_marker[consumed..];
    }
    proposals
}

fn extract_first_json_object(input: &str) -> Option<(String, usize)> {
    let start = input.find('{')?;
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;

    for (offset, ch) in input[start..].char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }

        match ch {
            '"' => in_string = true,
            '{' => depth += 1,
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    let end = start + offset + ch.len_utf8();
                    return Some((input[start..end].to_string(), end));
                }
            }
            _ => {}
        }
    }

    None
}
