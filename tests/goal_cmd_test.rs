use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
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

fn assert_goal_controller_artifacts(goal_dir: &std::path::Path) {
    assert!(goal_dir.join("prd.md").exists());
    assert!(goal_dir.join("technical-plan.md").exists());
    assert!(goal_dir.join("test-spec.md").exists());
    assert!(goal_dir.join("task-graph.json").exists());
    assert!(goal_dir.join("proof.json").exists());
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

fn git(project_dir: &std::path::Path, args: &[&str]) -> String {
    let output = StdCommand::new("git")
        .arg("-C")
        .arg(project_dir)
        .args(args)
        .output()
        .expect("failed to run git");
    assert!(
        output.status.success(),
        "git {:?} failed: stdout={} stderr={}",
        args,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
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
        .stdout(predicate::str::contains("plan"))
        .stdout(predicate::str::contains("proof"))
        .stdout(predicate::str::contains("verify"))
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
    assert_goal_controller_artifacts(&dirs[0]);

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
    assert_eq!(json["phase"], "proof");
    assert_eq!(json["artifacts"].as_array().unwrap().len(), 5);
}

#[test]
fn test_goal_run_writes_task_graph_and_not_ready_proof() {
    let (_tmp, envs) = isolated_env();

    omk_cmd(&envs)
        .args([
            "goal",
            "run",
            "Build a proof-backed goal controller scaffold",
            "--until-ready",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Proof:"))
        .stdout(predicate::str::contains("not_ready"));

    let dirs = goal_dirs(&envs);
    assert_eq!(dirs.len(), 1);
    assert_goal_controller_artifacts(&dirs[0]);

    let task_graph: Value = serde_json::from_str(
        &fs::read_to_string(dirs[0].join("task-graph.json")).expect("missing task graph"),
    )
    .expect("task graph should be JSON");
    assert!(task_graph["goal_id"].as_str().unwrap().starts_with("goal-"));
    assert_eq!(task_graph["tasks"].as_array().unwrap().len(), 3);
    assert_eq!(task_graph["tasks"][0]["id"], "goal-intake");
    assert_eq!(task_graph["tasks"][0]["status"], "done");
    assert_eq!(task_graph["tasks"][0]["owner_role"], "goal-controller");
    assert!(task_graph["tasks"][0]["completed_at"].as_str().is_some());
    assert!(task_graph["tasks"][0]["evidence"]
        .as_array()
        .unwrap()
        .iter()
        .any(|evidence| evidence["path"] == "prd.md"));
    assert_eq!(task_graph["tasks"][1]["id"], "goal-plan");
    assert_eq!(task_graph["tasks"][1]["status"], "done");
    assert!(task_graph["tasks"][1]["evidence"]
        .as_array()
        .unwrap()
        .iter()
        .any(|evidence| evidence["path"] == "technical-plan.md"));
    assert!(task_graph["tasks"][1]["evidence"]
        .as_array()
        .unwrap()
        .iter()
        .any(|evidence| evidence["path"] == "test-spec.md"));
    assert_eq!(task_graph["tasks"][2]["id"], "goal-execute-verify");
    assert_eq!(task_graph["tasks"][2]["status"], "pending");
    assert!(task_graph["tasks"][2]["evidence"]
        .as_array()
        .unwrap()
        .is_empty());

    let events = fs::read_to_string(dirs[0].join("events.jsonl")).expect("missing events");
    assert!(events.contains("\"kind\":\"task_completed\""));
    assert!(events.contains("\"actor\":\"goal-controller\""));

    let proof_output = omk_cmd(&envs)
        .args(["goal", "proof", "latest", "--json"])
        .output()
        .expect("omk goal proof failed");
    assert!(
        proof_output.status.success(),
        "proof failed: stdout={} stderr={}",
        String::from_utf8_lossy(&proof_output.stdout),
        String::from_utf8_lossy(&proof_output.stderr)
    );
    let proof_json: Value =
        serde_json::from_slice(&proof_output.stdout).expect("proof output should be JSON");
    assert_eq!(proof_json["status"], "not_ready");
    assert_eq!(proof_json["task_graph_summary"]["total_tasks"], 3);
    assert_eq!(proof_json["task_graph_summary"]["done_tasks"], 2);
    assert_eq!(proof_json["task_graph_summary"]["pending_tasks"], 1);
    assert!(proof_json["known_gaps"]
        .as_array()
        .unwrap()
        .iter()
        .any(|gap| gap
            .as_str()
            .unwrap()
            .contains("agent execution is not implemented")));
}

#[test]
fn test_goal_plan_creates_controller_scaffold_without_execution() {
    let (_tmp, envs) = isolated_env();

    omk_cmd(&envs)
        .args(["goal", "plan", "Prepare a migration proof plan"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Goal plan created"))
        .stdout(predicate::str::contains("not_ready"));

    let dirs = goal_dirs(&envs);
    assert_eq!(dirs.len(), 1);
    assert_goal_controller_artifacts(&dirs[0]);

    omk_cmd(&envs)
        .args(["goal", "proof", "latest", "--format", "md"])
        .assert()
        .success()
        .stdout(predicate::str::contains("# Goal Proof"))
        .stdout(predicate::str::contains("not_ready"));
}

#[test]
fn test_goal_verify_records_passing_gate_evidence_but_stays_not_ready() {
    let (_tmp, envs) = isolated_env();
    let project = tempfile::tempdir().expect("temp project");
    write_gate_config(project.path(), "smoke", "echo smoke-ok");

    let mut run = omk_cmd(&envs);
    run.current_dir(project.path())
        .args(["goal", "run", "Verify a tiny project"])
        .assert()
        .success();

    let mut verify = omk_cmd(&envs);
    verify
        .current_dir(project.path())
        .args(["goal", "verify", "latest"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Verification: not_ready"))
        .stdout(predicate::str::contains("smoke"))
        .stdout(predicate::str::contains("passed"));

    let dirs = goal_dirs(&envs);
    assert_eq!(dirs.len(), 1);
    let gate_artifacts = dirs[0].join("artifacts").join("gates");
    assert!(gate_artifacts.exists());
    assert!(fs::read_dir(gate_artifacts)
        .expect("gate artifacts should be readable")
        .any(|entry| entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .contains("smoke")));

    let proof_output = {
        let mut cmd = omk_cmd(&envs);
        cmd.current_dir(project.path())
            .args(["goal", "proof", "latest", "--json"])
            .output()
            .expect("omk goal proof failed")
    };
    assert!(proof_output.status.success());
    let proof_json: Value =
        serde_json::from_slice(&proof_output.stdout).expect("proof output should be JSON");
    assert_eq!(proof_json["status"], "not_ready");
    assert_eq!(proof_json["gates"].as_array().unwrap().len(), 1);
    assert_eq!(proof_json["gates"][0]["name"], "smoke");
    assert_eq!(proof_json["gates"][0]["passed"], true);
    assert!(!proof_json["known_gaps"]
        .as_array()
        .unwrap()
        .iter()
        .any(|gap| gap
            .as_str()
            .unwrap()
            .contains("verification gates have not run")));
    assert!(proof_json["known_gaps"]
        .as_array()
        .unwrap()
        .iter()
        .any(|gap| gap
            .as_str()
            .unwrap()
            .contains("agent execution is not implemented")));
}

#[test]
fn test_goal_verify_records_required_gate_failure() {
    let (_tmp, envs) = isolated_env();
    let project = tempfile::tempdir().expect("temp project");
    write_gate_config(project.path(), "smoke", "echo smoke-fail >&2; exit 7");

    let mut run = omk_cmd(&envs);
    run.current_dir(project.path())
        .args(["goal", "run", "Verify a failing tiny project"])
        .assert()
        .success();

    let mut verify = omk_cmd(&envs);
    verify
        .current_dir(project.path())
        .args(["goal", "verify", "latest"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Verification: not_ready"))
        .stdout(predicate::str::contains("smoke"))
        .stdout(predicate::str::contains("failed"));

    let proof_output = {
        let mut cmd = omk_cmd(&envs);
        cmd.current_dir(project.path())
            .args(["goal", "proof", "latest", "--json"])
            .output()
            .expect("omk goal proof failed")
    };
    assert!(proof_output.status.success());
    let proof_json: Value =
        serde_json::from_slice(&proof_output.stdout).expect("proof output should be JSON");
    assert_eq!(proof_json["status"], "not_ready");
    assert_eq!(proof_json["gates"][0]["name"], "smoke");
    assert_eq!(proof_json["gates"][0]["passed"], false);
    assert_eq!(proof_json["gates"][0]["exit_code"], 7);
    assert!(proof_json["known_gaps"]
        .as_array()
        .unwrap()
        .iter()
        .any(|gap| gap
            .as_str()
            .unwrap()
            .contains("required verification gates failed")));
}

#[test]
fn test_goal_verify_records_git_evidence() {
    let (_tmp, envs) = isolated_env();
    let project = tempfile::tempdir().expect("temp project");
    git(project.path(), &["init"]);
    git(project.path(), &["checkout", "-b", "proof-branch"]);
    fs::write(project.path().join("README.md"), "# tiny\n").expect("failed to write readme");
    write_gate_config(project.path(), "smoke", "echo smoke-ok");
    git(project.path(), &["add", "README.md", ".omk/gates.toml"]);
    git(
        project.path(),
        &[
            "-c",
            "user.name=OMK Test",
            "-c",
            "user.email=omk@example.invalid",
            "commit",
            "-m",
            "init",
        ],
    );
    let head = git(project.path(), &["rev-parse", "HEAD"]);

    let mut run = omk_cmd(&envs);
    run.current_dir(project.path())
        .args(["goal", "run", "Capture git evidence"])
        .assert()
        .success();

    let mut verify = omk_cmd(&envs);
    verify
        .current_dir(project.path())
        .args(["goal", "verify", "latest"])
        .assert()
        .success();

    let proof_output = {
        let mut cmd = omk_cmd(&envs);
        cmd.current_dir(project.path())
            .args(["goal", "proof", "latest", "--json"])
            .output()
            .expect("omk goal proof failed")
    };
    assert!(proof_output.status.success());
    let proof_json: Value =
        serde_json::from_slice(&proof_output.stdout).expect("proof output should be JSON");
    assert_eq!(proof_json["git"]["branch"], "proof-branch");
    assert_eq!(proof_json["git"]["head"], head);
    assert_eq!(proof_json["git"]["head"].as_str().unwrap().len(), 40);
    assert_eq!(proof_json["git"]["dirty"], false);
    assert!(proof_json["commits"]
        .as_array()
        .unwrap()
        .iter()
        .any(|commit| commit.as_str() == Some(head.as_str())));
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
