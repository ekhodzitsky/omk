use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};

use crate::git::GitRepo;
use crate::runtime::goal::git_ops::auto_rebase::{
    attempt_auto_rebase, ConflictClassification, RebaseOutcome,
};
use crate::runtime::goal::state::GOAL_ARTIFACTS_DIR;
use crate::runtime::goal::task_graph::{
    update_goal_task_delivery_metadata, GoalTaskDeliveryMetadataUpdate, GoalTaskDeliveryStatus,
};

const INTEGRATION_ARTIFACTS_DIR: &str = "integration";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoalMergeConflictCheckRequest {
    pub repo_dir: PathBuf,
    pub goal_dir: PathBuf,
    pub task_id: String,
    pub source_ref: String,
    pub target_ref: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoalMergeConflictEvidence {
    pub task_id: String,
    pub source_ref: String,
    pub target_ref: String,
    pub clean_merge: bool,
    pub conflicting_files: Vec<String>,
    pub command_line: String,
    pub stdout_summary: String,
    pub stderr_summary: String,
    pub artifact_path: PathBuf,
    pub conflict_classification: Option<ConflictClassification>,
}

pub async fn detect_goal_merge_conflicts(
    request: GoalMergeConflictCheckRequest,
) -> Result<GoalMergeConflictEvidence> {
    let repo = GitRepo::open(&request.repo_dir)
        .map_err(|e| anyhow::anyhow!("failed to open git repo: {e}"))?;
    let result = repo
        .merge_tree(&request.target_ref, &request.source_ref)
        .await
        .map_err(|e| anyhow::anyhow!("git merge-tree failed: {e}"))?;

    let mut clean_merge = !result.has_conflicts;
    let mut conflicting_files = result.conflict_files.clone();
    let mut rebase_error = None;
    let mut conflict_classification = None;

    if !clean_merge {
        match attempt_auto_rebase(
            &request.repo_dir,
            &request.source_ref,
            &request.target_ref,
        )
        .await
        {
            Ok((RebaseOutcome::Clean, classification)) => {
                conflict_classification = classification;
                match repo
                    .merge_tree(&request.target_ref, &request.source_ref)
                    .await
                {
                    Ok(recheck) => {
                        clean_merge = !recheck.has_conflicts;
                        conflicting_files = recheck.conflict_files.clone();
                    }
                    Err(e) => {
                        rebase_error = Some(format!("merge_tree_recheck_failed: {e}"));
                    }
                }
            }
            Ok((RebaseOutcome::ConflictUnresolvable, Some(classification))) => {
                conflict_classification = Some(classification.clone());
                rebase_error = Some(format!(
                    "auto-rebase could not resolve conflicts: {}",
                    match classification {
                        ConflictClassification::Safe { reason } => reason,
                        ConflictClassification::Unsafe { reason } => reason,
                    }
                ));
            }
            Ok((RebaseOutcome::ConflictUnresolvable, None)) => {
                rebase_error = Some("auto-rebase could not resolve conflicts".to_string());
            }
            Err(e) => {
                rebase_error = Some(format!("auto_rebase_failed: {e}"));
            }
        }
    }

    if !clean_merge && conflicting_files.is_empty() {
        anyhow::bail!("merge-tree detected conflicts but no conflicting files were parsed");
    }

    let artifact_path = conflict_artifact_path(&request.task_id)?;
    let stdout_summary = if clean_merge {
        "clean merge".to_string()
    } else {
        conflicting_files.join("\n")
    };
    let evidence = GoalMergeConflictEvidence {
        task_id: request.task_id.clone(),
        source_ref: request.source_ref.clone(),
        target_ref: request.target_ref.clone(),
        clean_merge,
        conflicting_files,
        command_line: format!(
            "git merge-tree {} {}",
            request.target_ref, request.source_ref
        ),
        stdout_summary,
        stderr_summary: String::new(),
        artifact_path,
        conflict_classification,
    };

    write_conflict_artifact(&request.goal_dir, &evidence).await?;
    record_conflict_delivery_metadata(&request.goal_dir, &evidence, rebase_error.as_deref())
        .await?;
    Ok(evidence)
}

fn conflict_artifact_path(task_id: &str) -> Result<PathBuf> {
    let task_component = super::normalize_identifier_component("task id", task_id)?;
    Ok(PathBuf::from(GOAL_ARTIFACTS_DIR)
        .join(INTEGRATION_ARTIFACTS_DIR)
        .join(format!("merge-conflict-{task_component}.json")))
}

async fn write_conflict_artifact(
    goal_dir: &Path,
    evidence: &GoalMergeConflictEvidence,
) -> Result<()> {
    let path = goal_dir.join(&evidence.artifact_path);
    let parent = path
        .parent()
        .context("merge conflict artifact path must have a parent")?;
    tokio::fs::create_dir_all(parent).await.with_context(|| {
        format!(
            "Failed to create merge conflict artifact directory: {}",
            parent.display()
        )
    })?;
    let json = serde_json::to_vec_pretty(evidence)?;
    crate::runtime::atomic::atomic_write(&path, &json)
        .await
        .with_context(|| {
            format!(
                "Failed to write merge conflict artifact: {}",
                path.display()
            )
        })
}

async fn record_conflict_delivery_metadata(
    goal_dir: &Path,
    evidence: &GoalMergeConflictEvidence,
    conflict_blocking_reason: Option<&str>,
) -> Result<()> {
    let status = if evidence.clean_merge {
        GoalTaskDeliveryStatus::ReadyForReview
    } else {
        GoalTaskDeliveryStatus::Blocked
    };
    let summary = if evidence.clean_merge {
        format!(
            "clean merge check passed for {} into {}",
            evidence.source_ref, evidence.target_ref
        )
    } else {
        format!(
            "merge conflict detected for {} into {}: {}",
            evidence.source_ref,
            evidence.target_ref,
            evidence.conflicting_files.join(", ")
        )
    };
    let mut extra = Map::<String, Value>::new();
    extra.insert(
        "merge_conflict_artifact".to_string(),
        json!(evidence.artifact_path.display().to_string()),
    );
    extra.insert(
        "merge_conflict_clean".to_string(),
        json!(evidence.clean_merge),
    );
    if let Some(ref classification) = evidence.conflict_classification {
        extra.insert(
            "conflict_classification".to_string(),
            json!(classification),
        );
    }

    update_goal_task_delivery_metadata(
        goal_dir,
        &evidence.task_id,
        GoalTaskDeliveryMetadataUpdate {
            verification_summary: Some(summary),
            status: Some(status),
            conflict_evidence_path: if evidence.clean_merge {
                None
            } else {
                Some(evidence.artifact_path.clone())
            },
            conflict_blocking_reason: conflict_blocking_reason.map(|s| s.to_string()),
            extra,
            ..GoalTaskDeliveryMetadataUpdate::default()
        },
    )
    .await?;
    Ok(())
}
