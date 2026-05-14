use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

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
    let output = super::git_output(
        &request.repo_dir,
        vec![
            OsString::from("merge-tree"),
            OsString::from("--write-tree"),
            OsString::from("--name-only"),
            OsString::from(&request.target_ref),
            OsString::from(&request.source_ref),
        ],
        "detect goal merge conflicts",
    )
    .await?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let clean_merge = output.status.success();
    let conflicting_files = if clean_merge {
        Vec::new()
    } else {
        parse_conflicting_files(&stdout)
    };
    if !clean_merge && conflicting_files.is_empty() {
        return Err(super::git_failure("detect goal merge conflicts", &output));
    }

    let artifact_path = conflict_artifact_path(&request.task_id)?;
    let evidence = GoalMergeConflictEvidence {
        task_id: request.task_id.clone(),
        source_ref: request.source_ref.clone(),
        target_ref: request.target_ref.clone(),
        clean_merge,
        conflicting_files,
        command_line: format!(
            "git merge-tree --write-tree --name-only {} {}",
            request.target_ref, request.source_ref
        ),
        stdout_summary: summarize_output(&stdout),
        stderr_summary: summarize_output(&stderr),
        artifact_path,
    };

    write_conflict_artifact(&request.goal_dir, &evidence).await?;
    record_conflict_delivery_metadata(&request.goal_dir, &evidence).await?;
    Ok(evidence)
}

fn parse_conflicting_files(stdout: &str) -> Vec<String> {
    stdout
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToString::to_string)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
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

fn summarize_output(output: &str) -> String {
    output
        .lines()
        .take(20)
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}
