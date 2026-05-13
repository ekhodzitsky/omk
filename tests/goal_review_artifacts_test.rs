use assert_cmd::Command;
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;
use tempfile::TempDir;

fn isolated_env() -> (TempDir, Vec<(&'static str, PathBuf)>) {
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

[[gates]]
name = "performance"
command = "/bin/sh"
args = ["-c", "echo perf-ok"]
required = true
"#,
    )
    .expect("failed to write gates.toml");
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

fn proof_json(envs: &[(&'static str, PathBuf)], project_dir: &Path) -> Value {
    let mut cmd = omk_cmd(envs);
    let output = cmd
        .current_dir(project_dir)
        .args(["goal", "proof", "latest", "--json"])
        .output()
        .expect("omk goal proof failed");
    assert!(
        output.status.success(),
        "goal proof failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("proof output should be JSON")
}

#[test]
fn goal_proof_stays_not_ready_when_required_review_artifacts_are_missing() {
    let (_tmp, envs) = isolated_env();
    let project = tempfile::tempdir().expect("temp project");
    write_gate_config(project.path());
    git(project.path(), &["init"]);
    git(project.path(), &["config", "user.email", "omk@example.com"]);
    git(project.path(), &["config", "user.name", "OMK Test"]);
    git(project.path(), &["add", "."]);
    git(project.path(), &["commit", "-m", "baseline"]);

    let mut run = omk_cmd(&envs);
    run.current_dir(project.path())
        .args(["goal", "run", "Prepare missing review artifact proof"])
        .assert()
        .success();

    let mut execute = omk_cmd(&envs);
    execute
        .env("MOCK_KIMI", mock_kimi_path())
        .env("MOCK_KIMI_WRITE_FILE", "agent-output.txt")
        .env("OMK_WIRE_WORKER_POLL_INTERVAL_MS", "50")
        .current_dir(project.path())
        .args(["goal", "execute", "latest"])
        .assert()
        .success();

    let proof = proof_json(&envs, project.path());
    assert_eq!(proof["status"], "not_ready");
    assert!(proof.get("review_artifacts").is_none());
    let gaps = proof["known_gaps"].as_array().unwrap();
    assert!(gaps
        .iter()
        .any(|gap| gap.as_str().unwrap().contains("review evidence")));
    assert!(gaps
        .iter()
        .any(|gap| gap.as_str().unwrap().contains("security review")));
}

#[test]
fn goal_review_records_typed_reviewer_artifacts_without_model_calls() {
    let (_tmp, envs) = isolated_env();
    let project = tempfile::tempdir().expect("temp project");
    write_gate_config(project.path());
    git(project.path(), &["init"]);
    git(project.path(), &["config", "user.email", "omk@example.com"]);
    git(project.path(), &["config", "user.name", "OMK Test"]);
    git(project.path(), &["add", "."]);
    git(project.path(), &["commit", "-m", "baseline"]);

    let mut run = omk_cmd(&envs);
    run.current_dir(project.path())
        .args(["goal", "run", "Capture typed review artifacts"])
        .assert()
        .success();

    let mut execute = omk_cmd(&envs);
    execute
        .env("MOCK_KIMI", mock_kimi_path())
        .env("MOCK_KIMI_WRITE_FILE", "agent-output.txt")
        .env("OMK_WIRE_WORKER_POLL_INTERVAL_MS", "50")
        .current_dir(project.path())
        .args(["goal", "execute", "latest"])
        .assert()
        .success();

    let mut review = omk_cmd(&envs);
    review
        .env_remove("MOCK_KIMI")
        .current_dir(project.path())
        .args(["goal", "review", "latest"])
        .assert()
        .success();

    let proof = proof_json(&envs, project.path());
    let artifacts = proof["review_artifacts"]
        .as_array()
        .expect("proof should include typed review artifacts");
    let by_pass: BTreeMap<_, _> = artifacts
        .iter()
        .map(|artifact| {
            (
                artifact["pass"].as_str().expect("review pass"),
                artifact["status"].as_str().expect("review status"),
            )
        })
        .collect();

    assert_eq!(by_pass.len(), 5);
    assert_eq!(by_pass.get("architect"), Some(&"passed"));
    assert_eq!(by_pass.get("code"), Some(&"passed"));
    assert_eq!(by_pass.get("test"), Some(&"passed"));
    assert_eq!(by_pass.get("security"), Some(&"passed"));
    assert_eq!(by_pass.get("performance"), Some(&"passed"));

    let dirs = goal_dirs(&envs);
    assert_eq!(dirs.len(), 1);
    let review_markdown = fs::read_to_string(dirs[0].join("artifacts/reviews/goal-review.md"))
        .expect("missing goal review artifact");
    assert!(review_markdown.contains("## Review Artifacts"));
    assert!(review_markdown.contains("| architect | passed |"));
    assert!(review_markdown.contains("| performance | passed |"));
}
