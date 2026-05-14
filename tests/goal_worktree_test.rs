use omk::runtime::goal::{
    materialize_goal_worktrees, plan_goal_worktree, plan_goal_worktrees,
    GoalWorktreeMaterializeRequest, GOAL_TASK_GRAPH_FILE,
};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[test]
fn test_goal_worktree_plan_is_deterministic_and_task_scoped() {
    let root = Path::new("/repo/.omk/worktrees");

    let first = plan_goal_worktree(root, "goal-20260513-155000-deadbeef", "omk-io2.2")
        .expect("worktree plan should be valid");
    let second = plan_goal_worktree(root, "goal-20260513-155000-deadbeef", "omk-io2.2")
        .expect("worktree plan should be repeatable");

    assert_eq!(first, second);
    assert_eq!(first.goal_id, "goal-20260513-155000-deadbeef");
    assert_eq!(first.task_id, "omk-io2.2");
    assert_eq!(
        first.branch_name,
        "omk/goal/goal-20260513-155000-deadbeef/omk-io2-2-7ec701dc2d4c52a1"
    );
    assert_eq!(
        first.worktree_name,
        "goal-goal-20260513-155000-deadbeef-omk-io2-2-7ec701dc2d4c52a1"
    );
    assert_eq!(first.worktree_path, root.join(&first.worktree_name));
}

#[test]
fn test_goal_worktree_plan_normalizes_unsafe_components() {
    let root = Path::new("/repo/.omk/worktrees");

    let plan = plan_goal_worktree(root, " ../Goal MVP ", "omk/io2:2")
        .expect("unsafe separators should normalize into safe names");

    assert_eq!(plan.goal_component, "goal-mvp");
    assert_eq!(plan.task_component, "omk-io2-2");
    assert!(!plan.branch_name.contains(".."));
    assert!(!plan.branch_name.contains(':'));
    assert!(!plan.branch_name.contains('\\'));
    assert!(!plan.worktree_name.contains('/'));
    assert!(!plan.worktree_name.contains('\\'));
}

#[test]
fn test_goal_worktree_plan_rejects_components_without_safe_text() {
    let root = Path::new("/repo/.omk/worktrees");

    let err = plan_goal_worktree(root, "../..", "omk-io2.2")
        .expect_err("path traversal only should not become a component");

    assert!(err.to_string().contains("goal id"));
}

#[test]
fn test_goal_worktree_plan_rejects_control_characters() {
    let root = Path::new("/repo/.omk/worktrees");

    let err = plan_goal_worktree(root, "goal-1", "task\n1")
        .expect_err("control characters should not normalize into identifiers");

    assert!(err.to_string().contains("task id"));
}

#[test]
fn test_goal_worktree_plan_avoids_normalized_identifier_collisions() {
    let root = Path::new("/repo/.omk/worktrees");

    let slash = plan_goal_worktree(root, "Goal MVP", "agent/implement")
        .expect("slash task should normalize");
    let colon = plan_goal_worktree(root, "Goal MVP", "agent:implement")
        .expect("colon task should normalize");

    assert_eq!(slash.goal_component, colon.goal_component);
    assert_eq!(slash.task_component, colon.task_component);
    assert_ne!(slash.branch_name, colon.branch_name);
    assert_ne!(slash.worktree_name, colon.worktree_name);
    assert_ne!(slash.worktree_path, colon.worktree_path);
}

#[test]
fn test_goal_worktree_batch_planner_rejects_duplicate_collisions() {
    let root = Path::new("/repo/.omk/worktrees");

    let err = plan_goal_worktrees(root, "Goal MVP", ["omk-io2.2", "omk-io2.2"])
        .expect_err("duplicate task plans should collide");

    assert!(err.to_string().contains("worktree plan collision"));
}

#[tokio::test]
async fn test_goal_worktree_dry_run_does_not_materialize_filesystem_or_branch() {
    let repo = temp_git_repo();
    let worktrees_root = repo.path().join("goal-worktrees");

    let outcome = materialize_goal_worktrees(materialize_request(
        repo.path(),
        &worktrees_root,
        None,
        true,
    ))
    .await
    .expect("dry-run materialization should plan successfully");

    assert!(outcome.dry_run);
    assert_eq!(outcome.plans.len(), 1);
    assert!(
        !worktrees_root.exists(),
        "dry-run must not create the worktrees root"
    );
    assert!(
        git_stdout(
            repo.path(),
            &["branch", "--list", &outcome.plans[0].branch_name]
        )
        .is_empty(),
        "dry-run must not create the planned branch"
    );
}

#[tokio::test]
async fn test_goal_worktree_materialize_rejects_existing_branch() {
    let repo = temp_git_repo();
    let worktrees_root = repo.path().join("goal-worktrees");
    let plan = plan_goal_worktree(&worktrees_root, "goal-materialize", "task-1")
        .expect("plan should be valid");
    run_git(repo.path(), &["branch", &plan.branch_name]);

    let err = materialize_goal_worktrees(materialize_request(
        repo.path(),
        &worktrees_root,
        None,
        false,
    ))
    .await
    .expect_err("existing branch should block materialization");

    assert!(err.to_string().contains("branch already exists"));
    assert!(err.to_string().contains(&plan.branch_name));
}

#[tokio::test]
async fn test_goal_worktree_materialize_rejects_existing_path() {
    let repo = temp_git_repo();
    let worktrees_root = repo.path().join("goal-worktrees");
    let plan = plan_goal_worktree(&worktrees_root, "goal-materialize", "task-1")
        .expect("plan should be valid");
    fs::create_dir_all(&plan.worktree_path).expect("existing worktree path");

    let err = materialize_goal_worktrees(materialize_request(
        repo.path(),
        &worktrees_root,
        None,
        false,
    ))
    .await
    .expect_err("existing path should block materialization");

    assert!(err.to_string().contains("worktree path already exists"));
    assert!(err
        .to_string()
        .contains(&plan.worktree_path.display().to_string()));
}

#[tokio::test]
async fn test_goal_worktree_materialize_creates_branch_and_worktree() {
    let repo = temp_git_repo();
    let worktrees_root = repo.path().join("goal-worktrees");

    let outcome = materialize_goal_worktrees(materialize_request(
        repo.path(),
        &worktrees_root,
        None,
        false,
    ))
    .await
    .expect("materialization should create a branch and worktree");
    let plan = &outcome.plans[0];

    assert!(!outcome.dry_run);
    assert!(plan.worktree_path.join(".git").exists());
    assert_eq!(
        git_stdout(&plan.worktree_path, &["branch", "--show-current"]),
        plan.branch_name
    );
    assert_eq!(
        git_stdout(repo.path(), &["rev-parse", "--verify", &plan.branch_name]),
        git_stdout(&plan.worktree_path, &["rev-parse", "HEAD"])
    );
}

#[tokio::test]
async fn test_goal_worktree_materialize_records_delivery_metadata_for_task() {
    let repo = temp_git_repo();
    let worktrees_root = repo.path().join("goal-worktrees");
    let state = tempfile::tempdir().expect("goal state tempdir");
    let goal_dir = state.path().join("goal-materialize");
    fs::create_dir_all(&goal_dir).expect("goal dir");
    let mut graph = task_graph_json();
    graph["tasks"][0]["delivery"] = json!({
        "verification_summary": "kept from earlier delivery metadata"
    });
    fs::write(
        goal_dir.join(GOAL_TASK_GRAPH_FILE),
        serde_json::to_vec_pretty(&graph).expect("task graph json"),
    )
    .expect("write task graph");

    let outcome = materialize_goal_worktrees(materialize_request(
        repo.path(),
        &worktrees_root,
        Some(goal_dir.clone()),
        false,
    ))
    .await
    .expect("materialization should record delivery metadata");
    let plan = &outcome.plans[0];
    let task_graph: Value = serde_json::from_slice(
        &fs::read(goal_dir.join(GOAL_TASK_GRAPH_FILE)).expect("read task graph"),
    )
    .expect("task graph json");
    let delivery = &task_graph["tasks"][0]["delivery"];

    assert_eq!(delivery["branch"], plan.branch_name);
    assert_eq!(delivery["owner"], "executor");
    assert_eq!(delivery["status"], "planned");
    assert_eq!(
        delivery["verification_summary"],
        "kept from earlier delivery metadata"
    );
    assert_eq!(
        delivery["write_scope"]
            .as_array()
            .expect("write scope")
            .len(),
        1
    );
    assert_eq!(
        delivery["worktree_path"],
        plan.worktree_path.display().to_string()
    );
}

#[tokio::test]
async fn test_goal_worktree_materialize_rejects_dirty_repo() {
    let repo = temp_git_repo();
    fs::write(repo.path().join("dirty.txt"), "untracked").expect("dirty file");

    let err = materialize_goal_worktrees(materialize_request(
        repo.path(),
        &repo.path().join("goal-worktrees"),
        None,
        false,
    ))
    .await
    .expect_err("dirty repo should block materialization");

    assert!(err.to_string().contains("requires a clean git worktree"));
}

#[tokio::test]
async fn test_goal_worktree_materialize_rejects_non_git_directory() {
    let repo = tempfile::tempdir().expect("non-git tempdir");

    let err = materialize_goal_worktrees(materialize_request(
        repo.path(),
        &repo.path().join("goal-worktrees"),
        None,
        false,
    ))
    .await
    .expect_err("non-git directory should block materialization");

    assert!(err.to_string().contains("requires a git repository"));
}

fn materialize_request(
    repo_dir: &Path,
    worktrees_root: &Path,
    goal_dir: Option<PathBuf>,
    dry_run: bool,
) -> GoalWorktreeMaterializeRequest {
    GoalWorktreeMaterializeRequest {
        repo_dir: repo_dir.to_path_buf(),
        worktrees_root: worktrees_root.to_path_buf(),
        goal_dir,
        goal_id: "goal-materialize".to_string(),
        task_ids: vec!["task-1".to_string()],
        dry_run,
    }
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

fn git_stdout(repo_dir: &Path, args: &[&str]) -> String {
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
    String::from_utf8(output.stdout)
        .expect("git stdout utf8")
        .trim()
        .to_string()
}

fn task_graph_json() -> Value {
    json!({
        "version": 1,
        "goal_id": "goal-materialize",
        "generated_at": "2026-05-14T00:00:00Z",
        "tasks": [{
            "id": "task-1",
            "title": "Task 1",
            "description": "Materialize worktree for task 1.",
            "status": "pending",
            "owner_role": "executor",
            "dependencies": [],
            "read_set": [],
            "write_set": ["src/runtime/goal/worktree.rs"],
            "risk": "medium",
            "acceptance": ["worktree metadata is recorded"]
        }]
    })
}
