use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

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

fn mock_kimi_path() -> PathBuf {
    assert_cmd::cargo::cargo_bin("mock-kimi")
}

fn write_gate_config(project_dir: &Path, gate_name: &str, script: &str) {
    let omk_dir = project_dir.join(".omk");
    fs::create_dir_all(&omk_dir).expect("failed to create .omk dir");
    fs::write(
        omk_dir.join("gates.toml"),
        format!(
            r#"
[[gates]]
name = "{gate_name}"
command = "/bin/sh"
args = ["-c", "{script}"]
required = true
"#
        ),
    )
    .expect("failed to write gates.toml");
}

fn init_git(project_dir: &Path) {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(project_dir)
        .arg("init")
        .output()
        .expect("failed to run git init");
    assert!(output.status.success());
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(project_dir)
        .args(["config", "user.email", "omk@example.com"])
        .output()
        .expect("failed to run git config");
    assert!(output.status.success());
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(project_dir)
        .args(["config", "user.name", "OMK Test"])
        .output()
        .expect("failed to run git config");
    assert!(output.status.success());
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(project_dir)
        .args(["add", "."])
        .output()
        .expect("failed to run git add");
    assert!(output.status.success());
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(project_dir)
        .args(["commit", "-m", "baseline"])
        .output()
        .expect("failed to run git commit");
    assert!(output.status.success());
}

fn goal_dirs(envs: &[(&'static str, PathBuf)]) -> Vec<PathBuf> {
    let goals_dir = envs
        .iter()
        .find_map(|(key, value)| (*key == "XDG_STATE_HOME").then(|| value.clone()))
        .expect("missing XDG_STATE_HOME")
        .join("omk")
        .join("goals");
    let mut dirs: Vec<_> = fs::read_dir(goals_dir)
        .expect("missing goals dir")
        .map(|entry| entry.expect("failed to read goal entry").path())
        .filter(|path| path.is_dir())
        .collect();
    dirs.sort();
    dirs
}

#[test]
fn run_until_ready_with_local_policy_stops_at_manual_integration_blocker() {
    let (_tmp, envs) = isolated_env();
    let project = tempfile::tempdir().expect("temp project");
    write_gate_config(
        project.path(),
        "acceptance-smoke-demo-performance",
        "echo controller-loop-ok",
    );
    init_git(project.path());

    let mut run = omk_cmd(&envs);
    run.env("MOCK_KIMI", mock_kimi_path())
        .env("MOCK_KIMI_WRITE_FILE", "agent-output.txt")
        .env("OMK_WIRE_WORKER_POLL_INTERVAL_MS", "50")
        .current_dir(project.path())
        .args([
            "goal",
            "run",
            "Implement acceptance smoke demo proof for this CLI",
            "--until-ready",
            "--policy",
            "local",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Goal run completed"))
        .stdout(predicate::str::contains("Controller steps:"))
        .stdout(predicate::str::contains("plan:"))
        .stdout(predicate::str::contains("verify:"))
        .stdout(predicate::str::contains("execute:"))
        .stdout(predicate::str::contains("review:"))
        .stdout(predicate::str::contains("manual integration acceptance"))
        .stdout(predicate::str::contains("GitHub mutation: disabled"));

    let dirs = goal_dirs(&envs);
    assert_eq!(dirs.len(), 1);
    let goal_dir = &dirs[0];
    assert!(goal_dir
        .join("artifacts/policy/manual-integration-blocker.json")
        .exists());

    let task_graph: Value = serde_json::from_str(
        &fs::read_to_string(goal_dir.join("task-graph.json")).expect("missing task graph"),
    )
    .expect("task graph should be JSON");
    for task_id in [
        "goal-local-verify",
        "goal-agent-execute",
        "goal-review",
        "goal-security-review",
    ] {
        let task = task_graph["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|task| task["id"] == task_id)
            .unwrap_or_else(|| panic!("missing {task_id}"));
        assert_eq!(task["status"], "done", "{task_id} should be done");
    }
}
