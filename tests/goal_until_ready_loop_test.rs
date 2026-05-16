use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;
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
    git(project_dir, &["init"]);
    git(project_dir, &["config", "user.email", "omk@example.com"]);
    git(project_dir, &["config", "user.name", "OMK Test"]);
    git(project_dir, &["add", "."]);
    git(project_dir, &["commit", "-m", "baseline"]);
}

fn git(project_dir: &Path, args: &[&str]) {
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
}

fn proof_json(envs: &[(&'static str, PathBuf)], project_dir: &Path) -> Value {
    let output = omk_cmd(envs)
        .current_dir(project_dir)
        .args(["goal", "proof", "latest", "--json"])
        .output()
        .expect("omk goal proof failed");
    assert!(output.status.success());
    serde_json::from_slice(&output.stdout).expect("proof output should be JSON")
}

fn read_jsonl(path: &Path) -> Vec<Value> {
    fs::read_to_string(path)
        .expect("missing jsonl file")
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).expect("jsonl line should parse"))
        .collect()
}

#[test]
fn run_until_ready_drives_controller_loop_and_stops_before_manual_acceptance() {
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
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Goal run completed"))
        .stdout(predicate::str::contains("Narrative:"))
        .stdout(predicate::str::contains("plan"))
        .stdout(predicate::str::contains("verify"))
        .stdout(predicate::str::contains("execute"))
        .stdout(predicate::str::contains("review"))
        .stdout(predicate::str::contains("manual integration acceptance"))
        .stdout(predicate::str::contains("GitHub mutation: disabled"))
        .stdout(predicate::str::contains("omk goal verify latest").not())
        .stdout(predicate::str::contains("omk goal execute latest").not())
        .stdout(predicate::str::contains("omk goal review latest").not());

    assert!(project.path().join("agent-output.txt").exists());
    let dirs = goal_dirs(&envs);
    assert_eq!(dirs.len(), 1);
    let goal_dir = &dirs[0];
    assert!(goal_dir
        .join("artifacts/policy/manual-integration-blocker.json")
        .exists());
    assert!(!goal_dir
        .join("artifacts/integration/integrator-accept.json")
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

    let proof = proof_json(&envs, project.path());
    assert_eq!(proof["status"], "not_ready");
    assert!(proof["changed_files"]
        .as_array()
        .unwrap()
        .iter()
        .any(|file| file.as_str() == Some("agent-output.txt")));
    assert!(proof["known_gaps"].as_array().unwrap().iter().any(|gap| gap
        .as_str()
        .is_some_and(|gap| gap.contains("manual integration acceptance"))));
    assert!(proof["human_decisions_required"]
        .as_array()
        .unwrap()
        .iter()
        .any(|decision| decision
            .as_str()
            .is_some_and(|decision| decision.contains("manual integration acceptance"))));
}

#[test]
fn run_until_ready_failed_gate_is_not_a_human_decision() {
    let (_tmp, envs) = isolated_env();
    let project = tempfile::tempdir().expect("temp project");
    write_gate_config(
        project.path(),
        "acceptance-smoke-demo-performance",
        "echo nope && exit 7",
    );
    init_git(project.path());

    let mut run = omk_cmd(&envs);
    run.current_dir(project.path())
        .args([
            "goal",
            "run",
            "Implement acceptance smoke demo proof for this CLI",
            "--until-ready",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Blocked: verification blocked"))
        .stdout(predicate::str::contains("Decision needed").not());

    let proof = proof_json(&envs, project.path());
    assert!(proof["known_gaps"].as_array().unwrap().iter().any(|gap| gap
        .as_str()
        .is_some_and(|gap| gap.contains("verification blocked"))));
    assert!(proof["human_decisions_required"]
        .as_array()
        .unwrap()
        .is_empty());
}

#[test]
fn run_until_ready_dispatches_accepted_agent_followup_before_review() {
    let (_tmp, envs) = isolated_env();
    let project = tempfile::tempdir().expect("temp project");
    write_gate_config(
        project.path(),
        "acceptance-smoke-demo-performance",
        "echo controller-followup-ok",
    );
    init_git(project.path());

    let proposal = r#"OMK_TASK_PROPOSAL: {"id":"goal-agent-docs-followup","title":"Document follow-up readiness","description":"Document the remaining readiness follow-up found by the agent wave.","dependencies":["goal-agent-execute"],"read_set":["README.md"],"write_set":["README.md"],"risk":"low","acceptance":["README captures the follow-up readiness gap."],"budget_secs":120}"#;
    let mut run = omk_cmd(&envs);
    run.env("MOCK_KIMI", mock_kimi_path())
        .env("MOCK_KIMI_WRITE_FILE", "agent-output.txt")
        .env("MOCK_KIMI_WIRE_TEXT_WHEN_CONTAINS", "goal-agent-implement")
        .env("MOCK_KIMI_WIRE_TEXT", proposal)
        .env("OMK_WIRE_WORKER_POLL_INTERVAL_MS", "50")
        .current_dir(project.path())
        .args([
            "goal",
            "run",
            "Implement acceptance smoke demo proof and follow-up docs",
            "--until-ready",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("execute").count(2))
        .stdout(predicate::str::contains("review"))
        .stdout(predicate::str::contains("manual integration acceptance"));

    let dirs = goal_dirs(&envs);
    assert_eq!(dirs.len(), 1);
    let goal_dir = &dirs[0];
    let followup_outbox = goal_dir
        .join("artifacts/agent-runs/goal-agent-followups/workers/goal-agent-worker-0/outbox.jsonl");
    let outbox = read_jsonl(&followup_outbox);
    assert!(outbox
        .iter()
        .any(|result| result["task_id"] == "goal-agent-docs-followup"));

    let task_graph: Value = serde_json::from_str(
        &fs::read_to_string(goal_dir.join("task-graph.json")).expect("missing task graph"),
    )
    .expect("task graph should be JSON");
    let followup = task_graph["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|task| task["id"] == "goal-agent-docs-followup")
        .expect("missing accepted follow-up task");
    assert_eq!(followup["status"], "done");
}

#[test]
fn run_until_ready_stops_on_blocked_review_wall_before_manual_integration() {
    let (_tmp, envs) = isolated_env();
    let project = tempfile::tempdir().expect("temp project");
    write_gate_config(
        project.path(),
        "acceptance-smoke-demo",
        "echo controller-review-wall-blocked",
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
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("performance review is blocked"))
        .stdout(predicate::str::contains("manual integration acceptance").not());

    let proof = proof_json(&envs, project.path());
    assert_eq!(proof["status"], "not_ready");
    assert!(proof["known_gaps"].as_array().unwrap().iter().any(|gap| gap
        .as_str()
        .is_some_and(|gap| gap.contains("performance review is blocked"))));
    assert!(proof["human_decisions_required"]
        .as_array()
        .unwrap()
        .is_empty());
}
