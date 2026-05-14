use omk::runtime::goal::{
    detect_goal_merge_conflicts, read_goal_task_delivery_metadata, GoalMergeConflictCheckRequest,
    GoalTaskDeliveryStatus, GOAL_TASK_GRAPH_FILE,
};
use serde_json::{json, Value};
use std::fs;
use std::path::Path;
use std::process::Command;

#[tokio::test]
async fn merge_conflict_detection_records_blocked_delivery_evidence() {
    let repo = temp_git_repo();
    let goal_dir = temp_goal_dir();
    run_git(repo.path(), &["checkout", "-b", "task-conflict"]);
    fs::write(repo.path().join("README.md"), "task branch\n").expect("task edit");
    run_git(repo.path(), &["commit", "-am", "task edit"]);
    run_git(repo.path(), &["checkout", "master"]);
    fs::write(repo.path().join("README.md"), "target branch\n").expect("target edit");
    run_git(repo.path(), &["commit", "-am", "target edit"]);

    let evidence = detect_goal_merge_conflicts(conflict_request(
        repo.path(),
        goal_dir.path(),
        "task-conflict",
        "task-conflict",
        "master",
    ))
    .await
    .expect("conflict detection should complete");

    assert!(!evidence.clean_merge);
    assert_eq!(evidence.task_id, "task-conflict");
    assert!(evidence
        .conflicting_files
        .iter()
        .any(|path| path == "README.md"));
    assert!(goal_dir.path().join(&evidence.artifact_path).exists());

    let metadata = read_goal_task_delivery_metadata(goal_dir.path(), "task-conflict")
        .await
        .expect("delivery metadata should read")
        .expect("delivery metadata should exist");
    assert_eq!(metadata.status, Some(GoalTaskDeliveryStatus::Blocked));
    assert!(metadata
        .verification_summary
        .as_deref()
        .is_some_and(|summary| summary.contains("merge conflict")));
}

#[tokio::test]
async fn clean_merge_detection_records_ready_for_review_delivery_evidence() {
    let repo = temp_git_repo();
    let goal_dir = temp_goal_dir();
    run_git(repo.path(), &["checkout", "-b", "task-clean"]);
    fs::write(repo.path().join("feature.txt"), "feature\n").expect("feature edit");
    run_git(repo.path(), &["add", "feature.txt"]);
    run_git(repo.path(), &["commit", "-m", "feature edit"]);
    run_git(repo.path(), &["checkout", "master"]);

    let evidence = detect_goal_merge_conflicts(conflict_request(
        repo.path(),
        goal_dir.path(),
        "task-clean",
        "task-clean",
        "master",
    ))
    .await
    .expect("clean merge detection should complete");

    assert!(evidence.clean_merge);
    assert!(evidence.conflicting_files.is_empty());
    assert!(goal_dir.path().join(&evidence.artifact_path).exists());

    let metadata = read_goal_task_delivery_metadata(goal_dir.path(), "task-clean")
        .await
        .expect("delivery metadata should read")
        .expect("delivery metadata should exist");
    assert_eq!(
        metadata.status,
        Some(GoalTaskDeliveryStatus::ReadyForReview)
    );
    assert!(metadata
        .verification_summary
        .as_deref()
        .is_some_and(|summary| summary.contains("clean merge")));
}

fn conflict_request(
    repo_dir: &Path,
    goal_dir: &Path,
    task_id: &str,
    source_ref: &str,
    target_ref: &str,
) -> GoalMergeConflictCheckRequest {
    GoalMergeConflictCheckRequest {
        repo_dir: repo_dir.to_path_buf(),
        goal_dir: goal_dir.to_path_buf(),
        task_id: task_id.to_string(),
        source_ref: source_ref.to_string(),
        target_ref: target_ref.to_string(),
    }
}

fn temp_goal_dir() -> tempfile::TempDir {
    let goal_dir = tempfile::tempdir().expect("goal tempdir");
    fs::write(
        goal_dir.path().join(GOAL_TASK_GRAPH_FILE),
        serde_json::to_vec_pretty(&task_graph_json()).expect("task graph json"),
    )
    .expect("write task graph");
    goal_dir
}

fn temp_git_repo() -> tempfile::TempDir {
    let repo = tempfile::tempdir().expect("temp repo");
    run_git(repo.path(), &["init"]);
    run_git(
        repo.path(),
        &["config", "user.email", "test@example.invalid"],
    );
    run_git(repo.path(), &["config", "user.name", "OMK Test"]);
    fs::write(repo.path().join("README.md"), "hello\n").expect("seed file");
    run_git(repo.path(), &["add", "README.md"]);
    run_git(repo.path(), &["commit", "-m", "seed"]);
    repo
}

fn run_git(repo_dir: &Path, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_dir)
        .args(args)
        .output()
        .expect("git command should run");
    assert!(
        output.status.success(),
        "git {:?} failed\nstdout:\n{}\nstderr:\n{}",
        args,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn task_graph_json() -> Value {
    json!({
        "version": 1,
        "goal_id": "goal-conflict",
        "generated_at": "2026-05-14T00:00:00Z",
        "tasks": [
            task_json("task-conflict"),
            task_json("task-clean")
        ]
    })
}

fn task_json(task_id: &str) -> Value {
    json!({
        "id": task_id,
        "title": task_id,
        "description": "Task under delivery conflict check.",
        "status": "pending",
        "dependencies": [],
        "read_set": [],
        "write_set": ["README.md"],
        "risk": "medium",
        "acceptance": ["merge conflict evidence is recorded"]
    })
}
