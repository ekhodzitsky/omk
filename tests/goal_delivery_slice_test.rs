use assert_cmd::Command;
use chrono::Utc;
use omk::runtime::goal::{
    plan_goal_delivery_slices, record_goal_delivery_slice_plan, GoalTask, GoalTaskGraph,
    GoalTaskStatus, GOAL_TASK_GRAPH_FILE,
};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

fn task(id: &str, owner: Option<&str>, dependencies: &[&str], write_set: &[&str]) -> GoalTask {
    GoalTask {
        id: id.to_string(),
        title: format!("Task {id}"),
        description: format!("Task {id} description"),
        status: GoalTaskStatus::Pending,
        owner_role: owner.map(str::to_string),
        completed_at: None,
        evidence: Vec::new(),
        retry_count: 0,
        max_retries: 0,
        lease_expires_at: None,
        dependencies: dependencies
            .iter()
            .map(|dependency| dependency.to_string())
            .collect(),
        read_set: vec!["SPEC.md".to_string(), "TODO.md".to_string()],
        write_set: write_set.iter().map(|path| path.to_string()).collect(),
        risk: "medium".to_string(),
        acceptance: vec![format!("Task {id} acceptance")],
    }
}

fn graph(tasks: Vec<GoalTask>) -> GoalTaskGraph {
    GoalTaskGraph {
        version: 1,
        goal_id: "goal-20260514-delivery".to_string(),
        generated_at: Utc::now(),
        tasks,
    }
}

#[ignore = "integration: uses real git or bash (#TODO)"]
#[test]
fn delivery_slice_plan_is_deterministic_and_pr_sized() {
    let root = Path::new("/repo/.omk/worktrees");
    let graph = graph(vec![
        task("goal-plan", Some("controller"), &[], &[]),
        task(
            "slice-ui",
            Some("executor"),
            &["goal-plan"],
            &["src/runtime/goal/planner.rs"],
        ),
        task(
            "slice-docs",
            Some("writer"),
            &["goal-plan"],
            &["docs/PROJECT_MAP.md"],
        ),
    ]);

    let first = plan_goal_delivery_slices(root, &graph).expect("slice plan");
    let second = plan_goal_delivery_slices(root, &graph).expect("slice plan repeat");

    assert_eq!(first, second);
    assert_eq!(first.slices.len(), 2);
    assert_eq!(first.slices[0].slice_id, "slice-docs");
    assert_eq!(first.slices[1].slice_id, "slice-ui");
    assert_eq!(first.slices[0].owner_role, "writer");
    assert_eq!(first.slices[1].owner_role, "executor");
    assert_eq!(
        first.slices[1].write_scope,
        vec!["src/runtime/goal/planner.rs".to_string()]
    );
    assert!(first.slices[1]
        .branch_name
        .starts_with("omk/goal/goal-20260514-delivery/slice-ui-"));
    assert_eq!(
        first.slices[1].worktree_path,
        root.join(&first.slices[1].worktree_name)
    );
    assert!(first.slices[1]
        .gates
        .contains(&"local_verification_wall".to_string()));
    assert!(first.slices[1]
        .review_needs
        .contains(&"code_review".to_string()));
}

#[ignore = "integration: uses real git or bash (#TODO)"]
#[test]
fn delivery_slice_plan_serializes_overlapping_write_scopes() {
    let root = Path::new("/repo/.omk/worktrees");
    let graph = graph(vec![
        task("slice-a", Some("executor"), &[], &["src/runtime/goal"]),
        task(
            "slice-b",
            Some("executor"),
            &[],
            &["src/runtime/goal/planner.rs"],
        ),
    ]);

    let plan = plan_goal_delivery_slices(root, &graph).expect("slice plan");

    assert_eq!(plan.slices.len(), 2);
    assert_eq!(plan.slices[0].slice_id, "slice-a");
    assert_eq!(plan.slices[1].slice_id, "slice-b");
    assert_eq!(plan.slices[1].dependencies, vec!["slice-a".to_string()]);
    assert_eq!(plan.overlap_serializations.len(), 1);
    assert_eq!(plan.overlap_serializations[0].blocked_slice_id, "slice-b");
    assert_eq!(plan.overlap_serializations[0].serializes_after, "slice-a");
    assert_eq!(
        plan.overlap_serializations[0].path,
        "src/runtime/goal/planner.rs"
    );
}

#[ignore = "integration: uses real git or bash (#TODO)"]
#[test]
fn delivery_slice_plan_does_not_duplicate_existing_overlap_order() {
    let root = Path::new("/repo/.omk/worktrees");
    let graph = graph(vec![
        task("slice-a", Some("executor"), &[], &["src/runtime/goal"]),
        task(
            "slice-b",
            Some("executor"),
            &["slice-a"],
            &["src/runtime/goal/planner.rs"],
        ),
    ]);

    let plan = plan_goal_delivery_slices(root, &graph).expect("slice plan");

    assert_eq!(plan.slices.len(), 2);
    assert_eq!(plan.slices[1].dependencies, vec!["slice-a".to_string()]);
    assert!(plan.overlap_serializations.is_empty());
}

#[ignore = "integration: uses real git or bash (#TODO)"]
#[tokio::test]
async fn delivery_slice_plan_persists_metadata_on_task_graph() {
    let goal_dir = tempfile::tempdir().expect("goal dir");
    let root = goal_dir.path().join("worktrees");
    let graph = graph(vec![task(
        "slice-code",
        Some("executor"),
        &[],
        &["src/runtime/goal/task_graph/delivery/slice.rs"],
    )]);
    let graph_path = goal_dir.path().join(GOAL_TASK_GRAPH_FILE);
    fs::write(
        &graph_path,
        serde_json::to_vec_pretty(&graph).expect("task graph json"),
    )
    .expect("write graph");

    let plan = plan_goal_delivery_slices(&root, &graph).expect("slice plan");
    record_goal_delivery_slice_plan(goal_dir.path(), &plan)
        .await
        .expect("persist delivery slice metadata");

    let persisted: Value =
        serde_json::from_slice(&fs::read(&graph_path).expect("read graph")).expect("graph json");
    let delivery = &persisted["tasks"][0]["delivery"];
    assert_eq!(delivery["slice_id"], "slice-code");
    assert_eq!(delivery["owner"], "executor");
    assert_eq!(delivery["status"], "planned");
    assert_eq!(
        delivery["read_scope"],
        serde_json::json!(["SPEC.md", "TODO.md"])
    );
    assert_eq!(
        delivery["write_scope"],
        serde_json::json!(["src/runtime/goal/task_graph/delivery/slice.rs"])
    );
    assert_eq!(delivery["branch"], plan.slices[0].branch_name);
    assert_eq!(
        delivery["worktree_path"],
        plan.slices[0].worktree_path.display().to_string()
    );
    assert!(delivery["gates"]
        .as_array()
        .expect("gates")
        .iter()
        .any(|gate| gate == "local_verification_wall"));
    assert!(delivery["review_needs"]
        .as_array()
        .expect("review needs")
        .iter()
        .any(|review| review == "anti_slop_review"));
}

#[ignore = "integration: uses real git or bash (#TODO)"]
#[test]
fn until_ready_goal_run_materializes_delivery_worktrees_without_pr_side_effects() {
    let (_xdg, envs) = omk::test_helpers::isolated_xdg_env();
    let repo = temp_git_repo();

    let mut cmd = Command::cargo_bin("omk").expect("omk binary");
    for (key, value) in &envs {
        cmd.env(key, value);
    }
    cmd.current_dir(repo.path())
        .args([
            "goal",
            "run",
            "Ship a deterministic delivery slice planner",
            "--until-ready",
        ])
        .assert()
        .success();

    let goal_dir = latest_goal_dir(&envs);
    let graph: Value = serde_json::from_slice(
        &fs::read(goal_dir.join(GOAL_TASK_GRAPH_FILE)).expect("read task graph"),
    )
    .expect("task graph json");
    let deliveries = graph["tasks"]
        .as_array()
        .expect("tasks")
        .iter()
        .filter_map(|task| task.get("delivery"))
        .collect::<Vec<_>>();

    assert!(
        !deliveries.is_empty(),
        "runtime should persist delivery metadata"
    );
    assert!(deliveries.iter().any(|delivery| {
        delivery
            .get("worktree_path")
            .and_then(Value::as_str)
            .is_some_and(|path| Path::new(path).join(".git").exists())
    }));
    assert!(deliveries
        .iter()
        .all(|delivery| delivery.get("commit_sha").is_none()));
    assert!(deliveries
        .iter()
        .all(|delivery| delivery.get("pr_url").is_none()));
}

#[ignore = "integration: uses real git or bash (#TODO)"]
#[test]
fn until_ready_goal_run_skips_worktree_materialization_on_dirty_baseline() {
    let (_xdg, envs) = omk::test_helpers::isolated_xdg_env();
    let repo = temp_git_repo();
    fs::write(repo.path().join("dirty.txt"), "untracked").expect("dirty file");

    let mut cmd = Command::cargo_bin("omk").expect("omk binary");
    for (key, value) in &envs {
        cmd.env(key, value);
    }
    cmd.current_dir(repo.path())
        .args([
            "goal",
            "run",
            "Ship a deterministic delivery slice planner",
            "--until-ready",
        ])
        .assert()
        .success();

    // Worktrees should NOT be materialized when the baseline is dirty.
    let goal_dir = latest_goal_dir(&envs);
    assert!(
        !goal_dir.join("worktrees").exists(),
        "worktrees should be skipped on dirty baseline"
    );
}

fn latest_goal_dir(envs: &[(&'static str, PathBuf)]) -> PathBuf {
    let goals_dir = envs
        .iter()
        .find_map(|(key, value)| (*key == "XDG_STATE_HOME").then(|| value.join("omk/goals")))
        .expect("state env");
    let mut dirs = fs::read_dir(goals_dir)
        .expect("goals dir")
        .map(|entry| entry.expect("goal entry").path())
        .filter(|path| path.is_dir())
        .collect::<Vec<_>>();
    dirs.sort();
    dirs.pop().expect("goal dir")
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
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(repo_dir)
        .args(args)
        .output()
        .expect("git command");
    assert!(
        output.status.success(),
        "git {:?} failed\nstdout:\n{}\nstderr:\n{}",
        args,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
