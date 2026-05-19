use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};

use crate::git::GitRepo;
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

    let clean_merge = !result.has_conflicts;
    let conflicting_files = result.conflict_files.clone();

    if !clean_merge && conflicting_files.is_empty() {
        anyhow::bail!("merge-tree detected conflicts but no conflicting files were parsed");
    }

    let artifact_path = conflict_artifact_path(&request.task_id)?;
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
        stdout_summary: if clean_merge {
            "clean merge".to_string()
        } else {
            result.conflict_files.join("\n")
        },
        stderr_summary: String::new(),
        artifact_path,
    };

    write_conflict_artifact(&request.goal_dir, &evidence).await?;
    record_conflict_delivery_metadata(&request.goal_dir, &evidence).await?;
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

    update_goal_task_delivery_metadata(
        goal_dir,
        &evidence.task_id,
        GoalTaskDeliveryMetadataUpdate {
            verification_summary: Some(summary),
            status: Some(status),
            extra,
            ..GoalTaskDeliveryMetadataUpdate::default()
        },
    )
    .await?;
    Ok(())
}


