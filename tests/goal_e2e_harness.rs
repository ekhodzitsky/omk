use assert_cmd::Command;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;

#[test]
fn test_goal_run_until_ready_first_user_path_records_progress_and_proof() {
    let (_tmp, envs) = isolated_env();
    let project = tempfile::tempdir().expect("temp project");
    write_gate_config(project.path());
    seed_git_project(project.path());

    omk_cmd(&envs)
        .current_dir(project.path())
        .args(["setup"])
        .assert()
        .success();
    commit_all_if_changed(project.path(), "record omk setup");

    let run = omk_cmd(&envs)
        .env("MOCK_KIMI", mock_kimi_path())
        .env("MOCK_KIMI_WRITE_FILE", "agent-output.txt")
        .env("MOCK_KIMI_WRITE_BODY", "north star goal e2e\n")
        .env("OMK_WIRE_WORKER_POLL_INTERVAL_MS", "50")
        .current_dir(project.path())
        .args([
            "goal",
            "run",
            "Build a tiny local Rust CLI fixture until it has proof-backed setup, terminal progress, and a clear readiness result",
            "--until-ready",
            "--budget-time",
            "30m",
            "--budget-tokens",
            "200000",
            "--max-agents",
            "1",
        ])
        .output()
        .expect("omk goal run should launch");
    assert!(
        run.status.success(),
        "goal run failed: stdout={} stderr={}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    let run_stdout = String::from_utf8(run.stdout).expect("run stdout should be UTF-8");
    assert_lines_present(
        &run_stdout,
        &fixture_lines("goal_end_to_end_progress_markers.txt"),
    );

    let show = command_json(&envs, project.path(), &["goal", "show", "latest", "--json"]);
    assert_eq!(show["status"], "not_ready");
    assert_eq!(show["phase"], "proof");
    assert_eq!(show["until_ready"], true);
    assert_eq!(show["budget_time"], "30m");
    assert_eq!(show["budget_tokens"], 200000);
    assert_eq!(show["max_agents"], 1);

    let replay = omk_cmd(&envs)
        .current_dir(project.path())
        .args(["goal", "replay", "latest", "--format", "text"])
        .output()
        .expect("omk goal replay should launch");
    assert!(
        replay.status.success(),
        "goal replay failed: stdout={} stderr={}",
        String::from_utf8_lossy(&replay.stdout),
        String::from_utf8_lossy(&replay.stderr)
    );
    let replay_stdout = String::from_utf8(replay.stdout).expect("replay stdout should be UTF-8");
    for marker in [
        "Goal replay",
        "Timeline:",
        "run_started",
        "budget_checkpoint",
    ] {
        assert!(
            replay_stdout.contains(marker),
            "replay output missing {marker}: {replay_stdout}"
        );
    }

    let proof = goal_proof_json(&envs, project.path());
    assert_eq!(proof["status"], "not_ready");
    assert!(proof["readiness"]
        .as_str()
        .is_some_and(|readiness| readiness.contains("integration acceptance")));
    assert_eq!(proof["post_mutation_gates_ran"], true);
    assert_json_array_contains_str(&proof["changed_files"], "agent-output.txt");
    for gap in fixture_lines("goal_end_to_end_known_gaps.txt") {
        assert_json_array_contains_str(&proof["known_gaps"], &gap);
    }

    let goal_dir = only_goal_dir(&envs);
    assert_goal_scaffold_artifacts(&goal_dir);
    assert!(goal_dir.join("events.jsonl").exists());
}

#[test]
fn test_goal_slice_execution_creates_worktrees_and_delivery_metadata() {
    let (_tmp, envs) = isolated_env();
    let project = tempfile::tempdir().expect("temp project");
    write_gate_config(project.path());
    seed_git_project(project.path());
    commit_all_if_changed(project.path(), "add gate config");

    let run = omk_cmd(&envs)
        .env("MOCK_KIMI", mock_kimi_path())
        .env("MOCK_KIMI_WRITE_FILE", "agent-output.txt")
        .env("MOCK_KIMI_WRITE_BODY", "slice-execution-e2e\n")
        .env("OMK_WIRE_WORKER_POLL_INTERVAL_MS", "50")
        .current_dir(project.path())
        .args([
            "goal",
            "run",
            "Build a tiny local Rust CLI fixture until it has proof-backed setup",
            "--until-ready",
            "--slice-execution",
        ])
        .output()
        .expect("omk goal run should launch");
    assert!(
        run.status.success(),
        "goal run failed: stdout={} stderr={}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    let run_stdout = String::from_utf8(run.stdout).expect("run stdout should be UTF-8");
    // Narrative section from orchestrator TUI (Iteration 4)
    assert!(
        run_stdout.contains("Narrative:"),
        "stdout should contain Narrative section: {run_stdout}"
    );

    let goal_dir = only_goal_dir(&envs);
    assert_goal_scaffold_artifacts(&goal_dir);

    // Verify worktrees directory exists
    let worktrees_dir = goal_dir.join("worktrees");
    assert!(
        worktrees_dir.exists(),
        "worktrees dir should exist: {}",
        worktrees_dir.display()
    );

    // Verify delivery records in task graph
    let task_graph: Value = serde_json::from_str(
        &fs::read_to_string(goal_dir.join("task-graph.json")).expect("missing task graph"),
    )
    .expect("task graph should be JSON");

    let tasks = task_graph["tasks"].as_array().expect("tasks array");
    let agent_tasks: Vec<&Value> = tasks
        .iter()
        .filter(|t| t.get("delivery").is_some())
        .collect();
    assert!(!agent_tasks.is_empty(), "should have agent tasks with delivery metadata for slices");

    // Each agent task should have delivery metadata with a real worktree path
    for task in &agent_tasks {
        let delivery = task.get("delivery").expect("task should have delivery metadata");
        assert!(delivery.get("slice_id").is_some(), "delivery should have slice_id");
        let wt_path = delivery
            .get("worktree_path")
            .and_then(|v| v.as_str())
            .expect("delivery should have worktree_path");
        assert!(
            std::path::Path::new(wt_path).exists(),
            "worktree_path should exist: {wt_path}"
        );
        assert!(delivery.get("branch").is_some(), "delivery should have branch");
        assert!(delivery.get("status").is_some(), "delivery should have status");
    }

    // Verify proof
    let proof = goal_proof_json(&envs, project.path());
    assert_eq!(proof["status"], "not_ready");
    // In slice-execution mode with mock kimi, changed_files may be empty
    // because the mock writes untracked files; the important contract is
    // that worktrees and delivery metadata were created.
}

#[test]
fn test_goal_concurrent_slice_execution_does_not_regress_with_max_agents() {
    let (_tmp, envs) = isolated_env();
    let project = tempfile::tempdir().expect("temp project");
    write_gate_config(project.path());
    seed_git_project(project.path());
    commit_all_if_changed(project.path(), "add gate config");

    // Use a multi-feature goal to trigger decomposition into non-overlapping slices
    let run = omk_cmd(&envs)
        .env("MOCK_KIMI", mock_kimi_path())
        .env("MOCK_KIMI_WRITE_FILE", "agent-output.txt")
        .env("MOCK_KIMI_WRITE_BODY", "concurrent-slice-e2e\n")
        .env("OMK_WIRE_WORKER_POLL_INTERVAL_MS", "50")
        .current_dir(project.path())
        .args([
            "goal",
            "run",
            "Build a tiny fixture with config parsing and logging",
            "--until-ready",
            "--slice-execution",
            "--max-agents",
            "2",
        ])
        .output()
        .expect("omk goal run should launch");
    assert!(
        run.status.success(),
        "goal run failed: stdout={} stderr={}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    let run_stdout = String::from_utf8(run.stdout).expect("run stdout should be UTF-8");
    assert!(
        run_stdout.contains("Narrative:"),
        "stdout should contain Narrative section: {run_stdout}"
    );

    let goal_dir = only_goal_dir(&envs);
    assert_goal_scaffold_artifacts(&goal_dir);

    // Worktrees should be created
    let worktrees_dir = goal_dir.join("worktrees");
    assert!(
        worktrees_dir.exists(),
        "worktrees dir should exist: {}",
        worktrees_dir.display()
    );

    // Verify multiple implement tasks were created and completed
    let task_graph: Value = serde_json::from_str(
        &fs::read_to_string(goal_dir.join("task-graph.json")).expect("missing task graph"),
    )
    .expect("task graph should be JSON");

    let tasks = task_graph["tasks"].as_array().expect("tasks array");
    let implement_tasks: Vec<&Value> = tasks
        .iter()
        .filter(|t| {
            t.get("id")
                .and_then(|v| v.as_str())
                .is_some_and(|id| id.starts_with("goal-agent-implement-"))
        })
        .collect();
    assert!(
        implement_tasks.len() >= 2,
        "should have at least 2 implement tasks for concurrent execution, found {}",
        implement_tasks.len()
    );

    for task in &implement_tasks {
        assert_eq!(
            task.get("status").and_then(|v| v.as_str()),
            Some("done"),
            "implement task {} should be done",
            task.get("id").and_then(|v| v.as_str()).unwrap_or("unknown")
        );
    }

    let has_delivery = tasks.iter().any(|t| t.get("delivery").is_some());
    assert!(has_delivery, "task graph should contain delivery metadata");
}

#[test]
fn test_goal_north_star_e2e_harness_reaches_open_pr_dry_run_render() {
    let (_tmp, envs) = isolated_env();
    let project = tempfile::tempdir().expect("temp project");
    write_gate_config(project.path());
    seed_git_project(project.path());

    omk_cmd(&envs)
        .current_dir(project.path())
        .args([
            "goal",
            "plan",
            "Implement a testable goal E2E marker file and verify the proof bundle",
        ])
        .assert()
        .success();

    let goal_dir = only_goal_dir(&envs);
    assert_goal_scaffold_artifacts(&goal_dir);

    omk_cmd(&envs)
        .current_dir(project.path())
        .args(["goal", "verify", "latest"])
        .assert()
        .success();

    omk_cmd(&envs)
        .env("MOCK_KIMI", mock_kimi_path())
        .env("MOCK_KIMI_WRITE_FILE", "agent-output.txt")
        .env("MOCK_KIMI_WRITE_BODY", "north star goal e2e\n")
        .env("OMK_WIRE_WORKER_POLL_INTERVAL_MS", "50")
        .current_dir(project.path())
        .args(["goal", "execute", "latest"])
        .assert()
        .success();

    omk_cmd(&envs)
        .current_dir(project.path())
        .args(["goal", "review", "latest"])
        .assert()
        .success();

    let proof = goal_proof_json(&envs, project.path());
    assert_eq!(proof["status"], "not_ready");
    assert_eq!(proof["post_mutation_gates_ran"], true);
    assert_eq!(
        proof["changed_files"],
        serde_json::json!(["agent-output.txt"])
    );
    assert_json_array_contains_str(
        &proof["known_gaps"],
        "integration loop has not committed, opened a PR, or accepted the agent changes yet",
    );
    assert!(goal_dir.join("proof.json").exists());
    assert!(goal_dir
        .join("artifacts/agent-runs/goal-agent-execute/mutation.diff")
        .exists());
    assert!(goal_dir.join("artifacts/reviews/goal-review.md").exists());
    assert!(goal_dir
        .join("artifacts/reviews/goal-security-review.md")
        .exists());

    let first_replay = goal_replay_json(&envs, project.path());
    let second_replay = goal_replay_json(&envs, project.path());
    assert_eq!(first_replay, second_replay);

    let open_pr = omk_cmd(&envs)
        .current_dir(project.path())
        .args(["goal", "open-pr", "latest", "--dry-run", "--format", "md"])
        .output()
        .expect("omk goal open-pr failed to launch");
    assert!(
        open_pr.status.success(),
        "open-pr dry-run failed: stdout={} stderr={}",
        String::from_utf8_lossy(&open_pr.stdout),
        String::from_utf8_lossy(&open_pr.stderr)
    );
    let pr_markdown = String::from_utf8(open_pr.stdout).expect("PR markdown should be UTF-8");
    assert!(pr_markdown.contains(
        "Title: Goal proof: Implement a testable goal E2E marker file and verify the proof bundle"
    ));
    assert!(pr_markdown.contains("## Goal"));
    assert!(pr_markdown.contains("agent-output.txt"));
    assert!(pr_markdown.contains("## Known Gaps"));
    assert!(pr_markdown.contains("integration loop has not committed"));
}

fn isolated_env() -> (tempfile::TempDir, Vec<(&'static str, PathBuf)>) {
    omk::test_helpers::isolated_xdg_env()
}

fn omk_cmd(envs: &[(&'static str, PathBuf)]) -> Command {
    let mut cmd = Command::cargo_bin("omk").expect("omk binary");
    for (key, value) in envs {
        cmd.env(key, value);
    }
    cmd
}

fn mock_kimi_path() -> PathBuf {
    assert_cmd::cargo::cargo_bin("mock-kimi")
}

fn seed_git_project(project_dir: &Path) {
    fs::write(
        project_dir.join("README.md"),
        "# Goal E2E Fixture\n\nThis fixture proves the user flow.\n",
    )
    .expect("write README");
    git(project_dir, &["init"]);
    git(project_dir, &["config", "user.email", "omk@example.com"]);
    git(project_dir, &["config", "user.name", "OMK Test"]);
    git(project_dir, &["add", "."]);
    git(project_dir, &["commit", "-m", "baseline"]);
}

fn commit_all_if_changed(project_dir: &Path, message: &str) {
    git(project_dir, &["add", "."]);
    let output = StdCommand::new("git")
        .arg("-C")
        .arg(project_dir)
        .args(["status", "--porcelain"])
        .output()
        .expect("git status should launch");
    assert!(
        output.status.success(),
        "git status failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    if output.stdout.is_empty() {
        return;
    }
    git(project_dir, &["commit", "-m", message]);
}

fn write_gate_config(project_dir: &Path) {
    let omk_dir = project_dir.join(".omk");
    fs::create_dir_all(&omk_dir).expect("create .omk");
    fs::write(
        omk_dir.join("gates.toml"),
        r#"
[[gates]]
name = "smoke"
command = "/bin/sh"
args = ["-c", "test -f README.md"]
required = true

[[gates]]
name = "perf-smoke"
command = "/bin/sh"
args = ["-c", "echo perf-ok"]
required = true
"#,
    )
    .expect("write gates.toml");
}

fn only_goal_dir(envs: &[(&'static str, PathBuf)]) -> PathBuf {
    let goals_dir = envs
        .iter()
        .find_map(|(key, value)| (*key == "XDG_STATE_HOME").then(|| value.join("omk/goals")))
        .expect("missing XDG_STATE_HOME");
    let mut dirs: Vec<_> = fs::read_dir(goals_dir)
        .expect("read goals dir")
        .map(|entry| entry.expect("goal dir entry").path())
        .filter(|path| path.is_dir())
        .collect();
    dirs.sort();
    assert_eq!(dirs.len(), 1);
    dirs.remove(0)
}

fn assert_goal_scaffold_artifacts(goal_dir: &Path) {
    for file in [
        "goal.json",
        "prd.md",
        "technical-plan.md",
        "test-spec.md",
        "task-graph.json",
        "proof.json",
    ] {
        assert!(
            goal_dir.join(file).exists(),
            "missing scaffold artifact {file}"
        );
    }
}

fn goal_proof_json(envs: &[(&'static str, PathBuf)], project_dir: &Path) -> Value {
    command_json(envs, project_dir, &["goal", "proof", "latest", "--json"])
}

fn goal_replay_json(envs: &[(&'static str, PathBuf)], project_dir: &Path) -> Value {
    command_json(envs, project_dir, &["goal", "replay", "latest", "--json"])
}

fn command_json(envs: &[(&'static str, PathBuf)], project_dir: &Path, args: &[&str]) -> Value {
    let output = omk_cmd(envs)
        .current_dir(project_dir)
        .args(args)
        .output()
        .expect("omk command failed to launch");
    assert!(
        output.status.success(),
        "omk {:?} failed: stdout={} stderr={}",
        args,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("command output should be JSON")
}

fn assert_json_array_contains_str(value: &Value, expected: &str) {
    assert!(
        value
            .as_array()
            .expect("value should be an array")
            .iter()
            .any(|item| item.as_str() == Some(expected)),
        "array should contain {expected}: {value}"
    );
}

fn fixture_lines(name: &str) -> Vec<String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name);
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read fixture {}: {error}", path.display()))
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(ToOwned::to_owned)
        .collect()
}

fn assert_lines_present(output: &str, lines: &[String]) {
    for line in lines {
        assert!(output.contains(line), "output missing {line}: {output}");
    }
}

fn git(project_dir: &Path, args: &[&str]) {
    let output = StdCommand::new("git")
        .arg("-C")
        .arg(project_dir)
        .args(args)
        .output()
        .expect("git command should launch");
    assert!(
        output.status.success(),
        "git {:?} failed: stdout={} stderr={}",
        args,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
