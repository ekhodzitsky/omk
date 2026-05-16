use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;

fn isolated_env() -> (tempfile::TempDir, Vec<(&'static str, PathBuf)>) {
    omk::test_helpers::isolated_xdg_env()
}

fn omk_cmd(envs: &[(&'static str, PathBuf)]) -> Command {
    let mut cmd = Command::cargo_bin("omk").unwrap();
    for (key, value) in envs {
        cmd.env(key, value);
    }
    cmd
}

fn xdg_state(envs: &[(&'static str, PathBuf)]) -> PathBuf {
    envs.iter()
        .find_map(|(key, value)| (*key == "XDG_STATE_HOME").then(|| value.clone()))
        .expect("missing XDG_STATE_HOME")
}

fn goal_dirs(envs: &[(&'static str, PathBuf)]) -> Vec<PathBuf> {
    let goals_dir = xdg_state(envs).join("omk").join("goals");
    let mut dirs: Vec<_> = fs::read_dir(goals_dir)
        .expect("missing goals dir")
        .map(|entry| entry.expect("failed to read goal entry").path())
        .filter(|path| path.is_dir())
        .collect();
    dirs.sort();
    dirs
}

fn git(project_dir: &Path, args: &[&str]) {
    let output = StdCommand::new("git")
        .arg("-C")
        .arg(project_dir)
        .args(args)
        .output()
        .expect("git command failed to start");
    assert!(
        output.status.success(),
        "git {:?} failed: stdout={} stderr={}",
        args,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn write_gate_config(project_dir: &Path) {
    let omk_dir = project_dir.join(".omk");
    fs::create_dir_all(&omk_dir).expect("failed to create .omk dir");
    fs::write(
        omk_dir.join("gates.toml"),
        r#"
[[gates]]
name = "smoke"
command = "/bin/sh"
args = ["-c", "echo smoke-ok"]
required = true
"#,
    )
    .expect("failed to write gates.toml");
}

fn init_project_with_goal(envs: &[(&'static str, PathBuf)]) -> (tempfile::TempDir, PathBuf) {
    let project = tempfile::tempdir().expect("project tempdir");
    git(project.path(), &["init"]);
    git(project.path(), &["config", "user.email", "omk@example.com"]);
    git(project.path(), &["config", "user.name", "OMK Test"]);
    fs::write(project.path().join("README.md"), "# fixture\n").expect("write readme");
    write_gate_config(project.path());
    git(project.path(), &["add", "."]);
    git(project.path(), &["commit", "-m", "baseline"]);
    git(
        project.path(),
        &["checkout", "-b", "codex/goal-open-pr-fixture"],
    );

    let mut plan = omk_cmd(envs);
    plan.current_dir(project.path())
        .args([
            "goal",
            "plan",
            "Render a GitHub PR from goal proof evidence",
        ])
        .assert()
        .success();

    let goal_dir = goal_dirs(envs)
        .into_iter()
        .next()
        .expect("goal dir should exist");
    inject_delivery_metadata(&goal_dir.join("task-graph.json"));
    fs::write(project.path().join("src-change.txt"), "changed\n").expect("write changed file");

    let mut verify = omk_cmd(envs);
    verify
        .current_dir(project.path())
        .args(["goal", "verify", "latest"])
        .assert()
        .success();

    (project, goal_dir)
}

fn inject_delivery_metadata(task_graph_path: &Path) {
    let mut task_graph: Value =
        serde_json::from_slice(&fs::read(task_graph_path).expect("read task graph"))
            .expect("task graph json");
    let task = task_graph["tasks"]
        .as_array_mut()
        .expect("task graph tasks should be an array")
        .iter_mut()
        .find(|task| task["id"] == "goal-agent-execute")
        .expect("generated graph should include agent execution task");
    task["delivery"] = json!({
        "slice_id": "goal-agent-execute",
        "owner": "codex",
        "branch": "codex/goal-open-pr-fixture",
        "worktree_path": "../oh-my-kimi-goal-open-pr",
        "pr_url": "https://github.com/ekhodzitsky/oh-my-kimi/pull/456",
        "write_scope": [
            "src/cli/goal/mod.rs",
            "src/cli/goal/commands/mod.rs",
            "src/runtime/goal/open_pr.rs",
            "tests/goal_open_pr_test.rs"
        ],
        "verification_summary": "cargo test --test goal_open_pr_test passed"
    });
    fs::write(
        task_graph_path,
        serde_json::to_vec_pretty(&task_graph).expect("task graph json"),
    )
    .expect("rewrite task graph");
}

#[test]
fn goal_help_lists_open_pr_command() {
    let (_tmp, envs) = isolated_env();

    omk_cmd(&envs)
        .args(["goal", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("open-pr"));
}

#[test]
fn goal_open_pr_markdown_dry_run_renders_goal_proof_and_delivery_metadata() {
    let (_tmp, envs) = isolated_env();
    let (_project, goal_dir) = init_project_with_goal(&envs);
    let goal_id = goal_dir
        .file_name()
        .and_then(|name| name.to_str())
        .expect("goal id");

    let output = omk_cmd(&envs)
        .args([
            "goal",
            "open-pr",
            goal_id,
            "--dry-run",
            "--format",
            "markdown",
        ])
        .output()
        .expect("omk goal open-pr failed");

    assert!(
        output.status.success(),
        "open-pr failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout utf8");
    assert!(stdout.contains("Title: Goal proof: Render a GitHub PR from goal proof evidence"));
    assert!(stdout.contains("## Goal"));
    assert!(stdout.contains(goal_id));
    assert!(stdout.contains("## Task Summary"));
    assert!(stdout.contains("## Delivery Metadata"));
    assert!(stdout.contains("slice: goal-agent-execute"));
    assert!(stdout.contains("owner: codex"));
    assert!(stdout.contains("branch: codex/goal-open-pr-fixture"));
    assert!(stdout.contains("worktree: ../oh-my-kimi-goal-open-pr"));
    assert!(stdout.contains("pr: https://github.com/ekhodzitsky/oh-my-kimi/pull/456"));
    assert!(stdout.contains("src/runtime/goal/open_pr.rs"));
    assert!(stdout.contains("## Proof Summary"));
    assert!(stdout.contains("Proof path:"));
    assert!(stdout.contains("proof.json"));
    assert!(stdout.contains("## Verification Wall"));
    assert!(stdout.contains("smoke"));
    assert!(!stdout.contains("smoke-ok"));
    assert!(stdout.contains("## Release Candidate Notes"));
    assert!(stdout.contains("merge recommendation"));
    assert!(stdout.contains("## Review Evidence"));
    assert!(stdout.contains("## Known Gaps"));
    assert!(stdout.contains("## Changed Files"));
    assert!(stdout.contains("src-change.txt"));
    assert!(stdout.contains("## Artifacts"));
}

#[test]
fn goal_open_pr_json_dry_run_is_valid_json() {
    let (_tmp, envs) = isolated_env();
    let (_project, _goal_dir) = init_project_with_goal(&envs);

    let output = omk_cmd(&envs)
        .args(["goal", "open-pr", "latest", "--dry-run", "--format", "json"])
        .output()
        .expect("omk goal open-pr failed");

    assert!(
        output.status.success(),
        "open-pr json failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout).expect("open-pr JSON should parse");
    assert_eq!(json["dry_run"], true);
    assert_eq!(
        json["title"],
        "Goal proof: Render a GitHub PR from goal proof evidence"
    );
    assert!(json["body"]
        .as_str()
        .expect("body string")
        .contains("## Verification Wall"));
}

#[test]
fn goal_open_pr_draft_dry_run_marks_draft_metadata() {
    let (_tmp, envs) = isolated_env();
    let (_project, _goal_dir) = init_project_with_goal(&envs);

    let output = omk_cmd(&envs)
        .args([
            "goal",
            "open-pr",
            "latest",
            "--dry-run",
            "--draft",
            "--format",
            "json",
        ])
        .output()
        .expect("omk goal open-pr failed");

    assert!(
        output.status.success(),
        "open-pr draft json failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout).expect("open-pr JSON should parse");
    assert_eq!(json["draft"], true);
    assert!(json["body"]
        .as_str()
        .expect("body string")
        .contains("- Draft: `true`"));
}

#[test]
fn goal_open_pr_default_local_policy_renders_without_dry_run() {
    let (_tmp, envs) = isolated_env();
    let (_project, _goal_dir) = init_project_with_goal(&envs);

    let output = omk_cmd(&envs)
        .args(["goal", "open-pr", "latest"])
        .output()
        .expect("omk goal open-pr failed");

    assert!(
        output.status.success(),
        "open-pr failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout utf8");
    assert!(stdout.contains("Title: Goal proof: Render a GitHub PR from goal proof evidence"));
}

#[test]
fn goal_open_pr_auto_pr_dry_run_renders_without_mutation() {
    let (_tmp, envs) = isolated_env();
    let (_project, _goal_dir) = init_project_with_goal(&envs);

    let output = omk_cmd(&envs)
        .args([
            "goal",
            "open-pr",
            "latest",
            "--policy",
            "auto-pr",
            "--dry-run",
            "--format",
            "json",
        ])
        .output()
        .expect("omk goal open-pr failed");

    assert!(
        output.status.success(),
        "open-pr auto-pr dry-run failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout).expect("open-pr JSON should parse");
    assert_eq!(json["dry_run"], true);
    assert_eq!(
        json["title"],
        "Goal proof: Render a GitHub PR from goal proof evidence"
    );
}

#[test]
fn goal_open_pr_local_policy_dry_run_false_is_same_as_default() {
    let (_tmp, envs) = isolated_env();
    let (_project, _goal_dir) = init_project_with_goal(&envs);

    let output = omk_cmd(&envs)
        .args(["goal", "open-pr", "latest", "--policy", "local"])
        .output()
        .expect("omk goal open-pr failed");

    assert!(
        output.status.success(),
        "open-pr local policy failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout utf8");
    assert!(stdout.contains("Title: Goal proof: Render a GitHub PR from goal proof evidence"));
}

#[test]
fn goal_open_pr_auto_pr_without_dry_run_fails_when_gh_not_authenticated() {
    let (_tmp, envs) = isolated_env();
    let (_project, _goal_dir) = init_project_with_goal(&envs);

    let output = omk_cmd(&envs)
        .args(["goal", "open-pr", "latest", "--policy", "auto-pr"])
        .output()
        .expect("omk goal open-pr failed");

    assert!(
        !output.status.success(),
        "open-pr auto-pr without dry-run should fail when gh is not authenticated"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    let helpful = stderr.contains("gh")
        || stderr.contains("failed")
        || stderr.contains("dry-run")
        || stderr.contains("dry_run");
    assert!(
        helpful,
        "stderr should contain a helpful message, got: {}",
        stderr
    );
}

#[test]
fn goal_open_pr_fails_when_proof_has_no_evidence() {
    let (_tmp, envs) = isolated_env();

    omk_cmd(&envs)
        .args(["goal", "plan", "Prepare a missing evidence PR"])
        .assert()
        .success();

    omk_cmd(&envs)
        .args(["goal", "open-pr", "latest", "--dry-run"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("proof evidence is missing"))
        .stderr(predicate::str::contains("Next: omk goal execute latest"));
}
