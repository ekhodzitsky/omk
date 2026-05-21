use omk::runtime::goal::{
    detect_goal_merge_conflicts, read_goal_task_delivery_metadata, GoalMergeConflictCheckRequest,
    GoalTaskDeliveryStatus, GOAL_TASK_GRAPH_FILE,
};
use serde_json::{json, Value};
use std::fs;
use std::path::Path;
use std::process::Command;

#[tokio::test]
async fn auto_rebase_not_triggered_when_merge_tree_clean() {
    let repo = temp_git_repo();
    let goal_dir = temp_goal_dir();
    run_git(repo.path(), &["checkout", "-b", "feature"]);
    fs::write(repo.path().join("feature.txt"), "feature\n").expect("feature edit");
    run_git(repo.path(), &["add", "feature.txt"]);
    run_git(repo.path(), &["commit", "-m", "feature edit"]);
    run_git(repo.path(), &["checkout", "master"]);
    let evidence = detect_goal_merge_conflicts(conflict_request(
        repo.path(),
        goal_dir.path(),
        "task-clean",
        "feature",
        "master",
    ))
    .await
    .expect("clean merge detection should complete");
    assert!(evidence.clean_merge);
    assert!(evidence.conflicting_files.is_empty());
    let metadata = read_goal_task_delivery_metadata(goal_dir.path(), "task-clean")
        .await
        .expect("read metadata")
        .expect("metadata exists");
    assert_eq!(
        metadata.status,
        Some(GoalTaskDeliveryStatus::ReadyForReview)
    );
    assert!(metadata.conflict_evidence_path.is_none());
    assert!(metadata.conflict_blocking_reason.is_none());
}

#[tokio::test]
async fn auto_rebase_resolves_conflict_and_clears_evidence() {
    let repo = temp_git_repo();
    let goal_dir = temp_goal_dir();
    run_git(repo.path(), &["checkout", "-b", "feature"]);
    fs::write(repo.path().join("file.txt"), "B\n").expect("c1");
    run_git(repo.path(), &["add", "file.txt"]);
    run_git(repo.path(), &["commit", "-m", "c1: B"]);
    fs::write(repo.path().join("file.txt"), "C\n").expect("c2");
    run_git(repo.path(), &["add", "file.txt"]);
    run_git(repo.path(), &["commit", "-m", "c2: C"]);
    run_git(repo.path(), &["checkout", "master"]);
    fs::write(repo.path().join("file.txt"), "B\n").expect("master");
    run_git(repo.path(), &["add", "file.txt"]);
    run_git(repo.path(), &["commit", "-m", "master: B"]);
    let evidence = detect_goal_merge_conflicts(conflict_request(
        repo.path(),
        goal_dir.path(),
        "task-resolved",
        "feature",
        "master",
    ))
    .await
    .expect("conflict detection should complete");
    assert!(evidence.clean_merge);
    assert!(evidence.conflicting_files.is_empty());
    let metadata = read_goal_task_delivery_metadata(goal_dir.path(), "task-resolved")
        .await
        .expect("read metadata")
        .expect("metadata exists");
    assert_eq!(
        metadata.status,
        Some(GoalTaskDeliveryStatus::ReadyForReview)
    );
    assert!(metadata.conflict_evidence_path.is_none());
    assert!(metadata.conflict_blocking_reason.is_none());
}

#[tokio::test]
async fn auto_rebase_fails_and_records_conflict_evidence() {
    let repo = temp_git_repo();
    let goal_dir = temp_goal_dir();
    run_git(repo.path(), &["checkout", "-b", "feature"]);
    fs::write(repo.path().join("shared.txt"), "feature content\n").expect("feature edit");
    run_git(repo.path(), &["add", "shared.txt"]);
    run_git(repo.path(), &["commit", "-m", "feature edit"]);
    run_git(repo.path(), &["checkout", "master"]);
    fs::write(repo.path().join("shared.txt"), "master content\n").expect("master edit");
    run_git(repo.path(), &["add", "shared.txt"]);
    run_git(repo.path(), &["commit", "-m", "master edit"]);
    let evidence = detect_goal_merge_conflicts(conflict_request(
        repo.path(),
        goal_dir.path(),
        "task-conflict",
        "feature",
        "master",
    ))
    .await
    .expect("conflict detection should complete");
    assert!(!evidence.clean_merge);
    assert!(evidence.conflicting_files.iter().any(|f| f == "shared.txt"));
    assert!(goal_dir.path().join(&evidence.artifact_path).exists());
    let metadata = read_goal_task_delivery_metadata(goal_dir.path(), "task-conflict")
        .await
        .expect("read metadata")
        .expect("metadata exists");
    assert_eq!(metadata.status, Some(GoalTaskDeliveryStatus::Blocked));
    assert!(metadata.conflict_evidence_path.is_some());
    assert!(metadata
        .conflict_blocking_reason
        .as_deref()
        .is_some_and(|r| r.contains("auto-rebase") || r.contains("auto_rebase")));
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
    fs::write(repo.path().join("file.txt"), "A\n").expect("seed file");
    run_git(repo.path(), &["add", "file.txt"]);
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
        "goal_id": "goal-auto-rebase",
        "generated_at": "2026-05-21T00:00:00Z",
        "tasks": [
            task_json("task-clean"),
            task_json("task-resolved"),
            task_json("task-conflict")
        ]
    })
}

fn task_json(task_id: &str) -> Value {
    json!({
        "id": task_id,
        "title": task_id,
        "description": "Task under auto-rebase conflict check.",
        "status": "pending",
        "dependencies": [],
        "read_set": [],
        "write_set": ["file.txt"],
        "risk": "medium",
        "acceptance": ["auto-rebase evidence is recorded correctly"]
    })
}
