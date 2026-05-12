use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;
use std::fs;
use std::path::PathBuf;

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

#[test]
fn test_goal_help_lists_goal_runtime_commands() {
    let (_tmp, envs) = isolated_env();

    omk_cmd(&envs)
        .args(["goal", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Goal runtime"))
        .stdout(predicate::str::contains("run"))
        .stdout(predicate::str::contains("status"))
        .stdout(predicate::str::contains("cancel"));
}

#[test]
fn test_goal_run_creates_durable_scaffold_and_show_json() {
    let (_tmp, envs) = isolated_env();

    omk_cmd(&envs)
        .args([
            "goal",
            "run",
            "Fix this repository until tests and proof pass",
            "--until-ready",
            "--budget-time",
            "8h",
            "--max-agents",
            "3",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Goal scaffold created"))
        .stdout(predicate::str::contains("not_ready"))
        .stdout(predicate::str::contains("omk goal show latest"));

    let dirs = goal_dirs(&envs);
    assert_eq!(dirs.len(), 1, "expected one goal dir, got {:?}", dirs);
    assert!(dirs[0].join("goal.json").exists());
    assert!(dirs[0].join("events.jsonl").exists());

    let output = omk_cmd(&envs)
        .args(["goal", "show", "latest", "--json"])
        .output()
        .expect("omk goal show failed");
    assert!(
        output.status.success(),
        "show failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let json: Value = serde_json::from_slice(&output.stdout).expect("show output should be JSON");
    assert_eq!(
        json["original_goal"],
        "Fix this repository until tests and proof pass"
    );
    assert_eq!(json["status"], "not_ready");
    assert_eq!(json["until_ready"], true);
    assert_eq!(json["budget_time"], "8h");
    assert_eq!(json["max_agents"], 3);
    assert_eq!(json["terminal_criteria"]["proof_required"], true);
}

#[test]
fn test_goal_status_list_and_cancel_latest() {
    let (_tmp, envs) = isolated_env();

    omk_cmd(&envs)
        .args(["goal", "run", "Ship the first goal state core"])
        .assert()
        .success();

    omk_cmd(&envs)
        .args(["goal", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Goals (1)"))
        .stdout(predicate::str::contains("Ship the first goal state core"))
        .stdout(predicate::str::contains("not_ready"));

    omk_cmd(&envs)
        .args(["goal", "status", "latest"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Goal status"))
        .stdout(predicate::str::contains("not_ready"));

    omk_cmd(&envs)
        .args(["goal", "cancel", "latest"])
        .assert()
        .success()
        .stdout(predicate::str::contains("cancelled"));

    let dirs = goal_dirs(&envs);
    assert_eq!(dirs.len(), 1);
    assert!(dirs[0].join("failure.json").exists());

    let output = omk_cmd(&envs)
        .args(["goal", "show", "latest", "--format", "json"])
        .output()
        .expect("omk goal show failed");
    assert!(output.status.success());

    let json: Value = serde_json::from_slice(&output.stdout).expect("show output should be JSON");
    assert_eq!(json["status"], "cancelled");
    assert_eq!(json["failure"]["reason"], "cancelled by user");
}
