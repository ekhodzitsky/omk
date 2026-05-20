use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;

#[test]
fn greenfield_goal_becomes_ready_after_integrator_accepts_oracle_evidence() {
    let (_tmp, envs) = omk::test_helpers::isolated_xdg_env();
    let project = ready_fixture_project(&[
        ("acceptance", "echo acceptance-ok"),
        ("smoke", "echo smoke-ok"),
        ("demo", "echo demo-ok"),
        ("performance", "echo perf-ok"),
    ]);

    run_goal_to_review(
        &envs,
        project.path(),
        "Build a greenfield CLI with acceptance smoke demo proof",
        "agent-output.txt",
        "greenfield ready fixture\n",
    );

    omk_cmd(&envs)
        .current_dir(project.path())
        .args([
            "goal",
            "accept",
            "latest",
            "--summary",
            "local integrator accepted greenfield fixture",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Acceptance: ready"));

    let proof = proof_json(&envs, project.path());
    assert_eq!(proof["status"], "ready");
    assert_eq!(
        proof["readiness"],
        "ready: integration and oracle evidence passed"
    );
    assert!(proof["known_gaps"]
        .as_array()
        .expect("known gaps")
        .is_empty());
    assert_eq!(proof["integration_evidence"]["status"], "accepted");
    assert_eq!(proof["oracle_evidence"]["kind"], "greenfield");
    assert_oracle_checks(&proof, &["acceptance", "smoke", "demo"]);

    let pr_draft = open_pr_markdown(&envs, project.path());
    assert!(pr_draft.contains("## Integration Evidence"));
    assert!(pr_draft.contains("status: accepted"));
    assert!(pr_draft.contains("## Oracle Evidence"));
    assert!(pr_draft.contains("kind: greenfield"));
    assert!(pr_draft.contains("acceptance"));
    assert!(pr_draft.contains("## Review Evidence"));
    assert!(pr_draft.contains("anti-slop"));
}

#[test]
fn rewrite_goal_becomes_ready_after_compatibility_oracle_acceptance() {
    let (_tmp, envs) = omk::test_helpers::isolated_xdg_env();
    let project = ready_fixture_project(&[
        ("compatibility", "echo compatibility-ok"),
        ("golden", "echo golden-ok"),
        ("performance", "echo perf-ok"),
    ]);

    run_goal_to_review(
        &envs,
        project.path(),
        "Rewrite the tiny Python CLI in Rust with compatibility golden proof",
        "rewrite-output.txt",
        "golden compatibility fixture\n",
    );

    omk_cmd(&envs)
        .current_dir(project.path())
        .args([
            "goal",
            "accept",
            "latest",
            "--summary",
            "local integrator accepted rewrite fixture",
        ])
        .assert()
        .success();

    let proof = proof_json(&envs, project.path());
    assert_eq!(proof["status"], "ready");
    assert_eq!(proof["oracle_evidence"]["kind"], "rewrite");
    assert_oracle_checks(&proof, &["compatibility", "golden"]);
}

#[test]
fn integrator_reject_keeps_goal_not_ready_with_visible_reason() {
    let (_tmp, envs) = omk::test_helpers::isolated_xdg_env();
    let project = ready_fixture_project(&[
        ("acceptance", "test -f agent-output.txt"),
        ("smoke", "true"),
        ("demo", "true"),
        ("performance", "true"),
    ]);
    run_goal_to_review(
        &envs,
        project.path(),
        "Build a greenfield CLI with acceptance smoke demo proof",
        "agent-output.txt",
        "greenfield ready fixture\n",
    );

    omk_cmd(&envs)
        .current_dir(project.path())
        .args([
            "goal",
            "reject",
            "latest",
            "--reason",
            "manual diff inspection found an unacceptable regression",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Acceptance: not_ready"));

    let proof = proof_json(&envs, project.path());
    assert_eq!(proof["status"], "not_ready");
    assert_eq!(proof["integration_evidence"]["status"], "rejected");
    assert!(proof["artifacts"]
        .as_array()
        .is_some_and(|artifacts| artifacts
            .iter()
            .any(|artifact| artifact["kind"] == "integration_rollback_plan")));
    assert_json_array_contains(
        &proof["known_gaps"],
        "manual diff inspection found an unacceptable regression",
    );
}

fn run_goal_to_review(
    envs: &[(&'static str, PathBuf)],
    project_dir: &Path,
    goal: &str,
    write_file: &str,
    write_body: &str,
) {
    let run_output = omk_cmd(envs)
        .current_dir(project_dir)
        .args(["goal", "run", goal])
        .output()
        .expect("omk goal run");
    eprintln!(
        "omk goal run stdout: {}",
        String::from_utf8_lossy(&run_output.stdout)
    );
    eprintln!(
        "omk goal run stderr: {}",
        String::from_utf8_lossy(&run_output.stderr)
    );
    assert!(run_output.status.success(), "omk goal run failed");

    let exec_output = omk_cmd(envs)
        .env("MOCK_KIMI", assert_cmd::cargo::cargo_bin("mock-kimi"))
        .env("MOCK_KIMI_WRITE_FILE", write_file)
        .env("MOCK_KIMI_WRITE_BODY", write_body)
        .env("OMK_WIRE_WORKER_POLL_INTERVAL_MS", "50")
        .current_dir(project_dir)
        .args(["goal", "execute", "latest"])
        .output()
        .expect("omk goal execute");
    eprintln!(
        "omk goal execute stdout: {}",
        String::from_utf8_lossy(&exec_output.stdout)
    );
    eprintln!(
        "omk goal execute stderr: {}",
        String::from_utf8_lossy(&exec_output.stderr)
    );
    assert!(exec_output.status.success(), "omk goal execute failed");

    omk_cmd(envs)
        .current_dir(project_dir)
        .args(["goal", "review", "latest"])
        .assert()
        .success();
}

fn ready_fixture_project(gates: &[(&str, &str)]) -> tempfile::TempDir {
    let project = tempfile::tempdir().expect("temp project");
    fs::write(project.path().join("README.md"), "# ready fixture\n").expect("write README");
    write_gate_config(project.path(), gates);
    git(project.path(), &["init"]);
    git(project.path(), &["config", "user.email", "omk@example.com"]);
    git(project.path(), &["config", "user.name", "OMK Test"]);
    git(project.path(), &["add", "."]);
    git(project.path(), &["commit", "-m", "baseline"]);
    project
}

fn write_gate_config(project_dir: &Path, gates: &[(&str, &str)]) {
    let omk_dir = project_dir.join(".omk");
    fs::create_dir_all(&omk_dir).expect("create .omk");
    let mut config = String::new();
    for (name, script) in gates {
        config.push_str(&format!(
            "\n[[gates]]\nname = \"{name}\"\ncommand = \"/bin/sh\"\nargs = [\"-c\", \"{script}\"]\nrequired = true\n"
        ));
    }
    fs::write(omk_dir.join("gates.toml"), config).expect("write gates config");
}

fn omk_cmd(envs: &[(&'static str, PathBuf)]) -> Command {
    let mut cmd = Command::cargo_bin("omk").expect("omk binary");
    for (key, value) in envs {
        cmd.env(key, value);
    }
    cmd
}

fn proof_json(envs: &[(&'static str, PathBuf)], project_dir: &Path) -> Value {
    let output = omk_cmd(envs)
        .current_dir(project_dir)
        .args(["goal", "proof", "latest", "--json"])
        .output()
        .expect("goal proof command");
    assert!(
        output.status.success(),
        "goal proof failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("proof json")
}

fn open_pr_markdown(envs: &[(&'static str, PathBuf)], project_dir: &Path) -> String {
    let output = omk_cmd(envs)
        .current_dir(project_dir)
        .args(["goal", "open-pr", "latest", "--dry-run", "--format", "md"])
        .output()
        .expect("goal open-pr command");
    assert!(
        output.status.success(),
        "goal open-pr failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("open-pr markdown")
}

fn assert_oracle_checks(proof: &Value, expected: &[&str]) {
    let checks = proof["oracle_evidence"]["checks"]
        .as_array()
        .expect("oracle checks");
    for expected_check in expected {
        assert!(
            checks.iter().any(|check| {
                check["name"]
                    .as_str()
                    .is_some_and(|name| name == *expected_check)
                    && check["status"].as_str() == Some("passed")
            }),
            "missing oracle check {expected_check}: {checks:#?}"
        );
    }
}

fn assert_json_array_contains(value: &Value, expected: &str) {
    assert!(
        value
            .as_array()
            .expect("json array")
            .iter()
            .any(|item| item.as_str() == Some(expected)),
        "expected {expected} in {value:#?}"
    );
}

fn git(project_dir: &Path, args: &[&str]) {
    let output = StdCommand::new("git")
        .arg("-C")
        .arg(project_dir)
        .args(args)
        .output()
        .expect("git command");
    assert!(
        output.status.success(),
        "git {:?} failed: stdout={} stderr={}",
        args,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
