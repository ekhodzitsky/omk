use assert_cmd::Command;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;

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
