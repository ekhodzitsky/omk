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

fn write_smoke_and_performance_gate_config(project_dir: &Path) {
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

fn write_smoke_only_gate_config(project_dir: &Path) {
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

fn proof_markdown(envs: &[(&'static str, PathBuf)], project_dir: &Path) -> String {
    let mut cmd = omk_cmd(envs);
    let output = cmd
        .current_dir(project_dir)
        .args(["goal", "proof", "latest", "--format", "md"])
        .output()
        .expect("omk goal proof markdown failed");
    assert!(
        output.status.success(),
        "goal proof markdown failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("proof markdown should be utf-8")
}

fn latest_goal_dir(envs: &[(&'static str, PathBuf)]) -> PathBuf {
    let state_home = envs
        .iter()
        .find(|(key, _)| *key == "XDG_STATE_HOME")
        .map(|(_, value)| value)
        .expect("isolated env should include XDG_STATE_HOME");
    let goals_dir = state_home.join("omk").join("goals");
    let mut dirs = fs::read_dir(&goals_dir)
        .expect("goals dir should exist")
        .map(|entry| entry.expect("goal dir entry").path())
        .filter(|path| path.is_dir())
        .collect::<Vec<_>>();
    dirs.sort();
    dirs.pop().expect("goal dir should exist")
}

fn init_git_project(project_dir: &Path) {
    git(project_dir, &["init"]);
    git(project_dir, &["config", "user.email", "omk@example.com"]);
    git(project_dir, &["config", "user.name", "OMK Test"]);
    git(project_dir, &["add", "."]);
    git(project_dir, &["commit", "-m", "baseline"]);
}

fn execute_goal_with_agent_evidence(
    envs: &[(&'static str, PathBuf)],
    project_dir: &Path,
    goal: &str,
) {
    let mut run = omk_cmd(envs);
    run.current_dir(project_dir)
        .args(["goal", "run", goal])
        .assert()
        .success();

    let mut execute = omk_cmd(envs);
    execute
        .env("MOCK_KIMI", mock_kimi_path())
        .env("MOCK_KIMI_WRITE_FILE", "agent-output.txt")
        .env("OMK_WIRE_WORKER_POLL_INTERVAL_MS", "50")
        .current_dir(project_dir)
        .args(["goal", "execute", "latest"])
        .assert()
        .success();
}

fn run_goal_review(envs: &[(&'static str, PathBuf)], project_dir: &Path) {
    let mut review = omk_cmd(envs);
    review
        .env_remove("MOCK_KIMI")
        .current_dir(project_dir)
        .args(["goal", "review", "latest"])
        .assert()
        .success();
}

#[test]
fn goal_proof_stays_not_ready_when_required_review_artifacts_are_missing() {
    let (_tmp, envs) = isolated_env();
    let project = tempfile::tempdir().expect("temp project");
    write_smoke_and_performance_gate_config(project.path());
    init_git_project(project.path());
    execute_goal_with_agent_evidence(
        &envs,
        project.path(),
        "Prepare missing review artifact proof",
    );

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
    write_smoke_and_performance_gate_config(project.path());
    init_git_project(project.path());
    execute_goal_with_agent_evidence(&envs, project.path(), "Capture typed review artifacts");
    run_goal_review(&envs, project.path());

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

    assert_eq!(by_pass.len(), 6);
    assert_eq!(by_pass.get("architect"), Some(&"passed"));
    assert_eq!(by_pass.get("code"), Some(&"passed"));
    assert_eq!(by_pass.get("test"), Some(&"passed"));
    assert_eq!(by_pass.get("security"), Some(&"passed"));
    assert_eq!(by_pass.get("performance"), Some(&"passed"));
    assert_eq!(by_pass.get("anti-slop"), Some(&"passed"));

    for artifact in artifacts {
        assert!(
            artifact.get("status").is_some(),
            "review artifact missing status: {artifact}"
        );
        assert!(
            artifact.get("evidence").and_then(Value::as_array).is_some(),
            "review artifact missing evidence array: {artifact}"
        );
        assert!(
            artifact.get("risks").and_then(Value::as_array).is_some(),
            "review artifact missing risks array: {artifact}"
        );
        assert!(
            artifact
                .get("known_gaps")
                .and_then(Value::as_array)
                .is_some(),
            "review artifact missing known_gaps array: {artifact}"
        );
        assert!(
            artifact
                .get("recommended_next_step")
                .and_then(Value::as_str)
                .is_some_and(|step| !step.trim().is_empty()),
            "review artifact missing recommended_next_step: {artifact}"
        );
    }
}

#[test]
fn goal_review_blocks_proof_when_performance_artifact_is_blocked() {
    let (_tmp, envs) = isolated_env();
    let project = tempfile::tempdir().expect("temp project");
    write_smoke_only_gate_config(project.path());
    init_git_project(project.path());
    execute_goal_with_agent_evidence(
        &envs,
        project.path(),
        "Keep blocked performance review visible",
    );
    run_goal_review(&envs, project.path());

    let proof = proof_json(&envs, project.path());
    let artifacts = proof["review_artifacts"]
        .as_array()
        .expect("proof should include typed review artifacts");
    let performance = artifacts
        .iter()
        .find(|artifact| artifact["pass"] == "performance")
        .expect("missing performance review artifact");
    assert_eq!(performance["status"], "blocked");

    let gaps = proof["known_gaps"].as_array().unwrap();
    assert!(gaps.iter().any(|gap| gap
        .as_str()
        .unwrap()
        .contains("performance review is blocked")));
    assert!(
        !proof["readiness"]
            .as_str()
            .unwrap()
            .contains("review, and security evidence"),
        "proof readiness must not claim blocked review evidence passed: {}",
        proof["readiness"]
    );
}

#[test]
fn goal_review_wall_json_and_markdown_are_stable_and_human_readable() {
    let (_tmp, envs) = isolated_env();
    let project = tempfile::tempdir().expect("temp project");
    write_smoke_and_performance_gate_config(project.path());
    init_git_project(project.path());
    execute_goal_with_agent_evidence(&envs, project.path(), "Render review wall proof");
    run_goal_review(&envs, project.path());

    let proof = proof_json(&envs, project.path());
    let artifacts = proof["review_artifacts"]
        .as_array()
        .expect("proof should include review artifacts");
    let passes = artifacts
        .iter()
        .map(|artifact| artifact["pass"].as_str().expect("pass name"))
        .collect::<Vec<_>>();
    assert_eq!(
        passes,
        vec![
            "architect",
            "code",
            "test",
            "security",
            "performance",
            "anti-slop"
        ]
    );

    let proof_md = proof_markdown(&envs, project.path());
    assert!(proof_md.contains("# Goal Proof "));
    assert!(proof_md.contains("## Known Gaps"));

    let review_md = fs::read_to_string(
        latest_goal_dir(&envs)
            .join("artifacts")
            .join("reviews")
            .join("goal-review.md"),
    )
    .expect("review artifact markdown should exist");
    assert!(review_md.contains("## Architect"));
    assert!(review_md.contains("## Code"));
    assert!(review_md.contains("## Test"));
    assert!(review_md.contains("## Security"));
    assert!(review_md.contains("## Performance"));
    assert!(review_md.contains("## Anti-Slop"));
    assert!(review_md.contains("Recommended next step:"));
}
