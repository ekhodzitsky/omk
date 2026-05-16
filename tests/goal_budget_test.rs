use assert_cmd::Command;
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

fn goal_dirs(envs: &[(&'static str, PathBuf)]) -> Vec<PathBuf> {
    let goals_dir = envs
        .iter()
        .find_map(|(key, value)| (*key == "XDG_STATE_HOME").then(|| value.clone()))
        .expect("missing XDG_STATE_HOME")
        .join("omk")
        .join("goals");
    let mut dirs: Vec<_> = fs::read_dir(&goals_dir)
        .expect("missing goals dir")
        .map(|entry| entry.expect("failed to read goal entry").path())
        .filter(|path| path.is_dir())
        .collect();
    dirs.sort();
    dirs
}

fn write_gate_config(project_dir: &std::path::Path, gate_name: &str, script: &str) {
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

#[test]
fn cost_tracker_records_phase_durations() {
    let (_tmp, envs) = isolated_env();
    let project = tempfile::tempdir().expect("temp project");
    write_gate_config(project.path(), "smoke", "echo smoke-ok");

    omk_cmd(&envs)
        .current_dir(project.path())
        .args([
            "goal",
            "run",
            "Track cost during verify",
            "--budget-time",
            "1h",
        ])
        .assert()
        .success();

    omk_cmd(&envs)
        .current_dir(project.path())
        .args(["goal", "verify", "latest"])
        .assert()
        .success();

    let dirs = goal_dirs(&envs);
    assert_eq!(dirs.len(), 1);
    let cost_path = dirs[0].join("cost.jsonl");
    assert!(cost_path.exists(), "cost.jsonl should exist after verify");

    let costs: Vec<Value> =
        serde_json::from_str(&fs::read_to_string(&cost_path).expect("read cost.jsonl"))
            .expect("parse cost.jsonl");

    assert!(
        costs.iter().any(|c| c["session_type"] == "verify"),
        "cost.jsonl should contain a verify entry: {costs:?}"
    );
}
