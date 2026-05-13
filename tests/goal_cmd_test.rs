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

fn read_jsonl(path: &std::path::Path) -> Vec<Value> {
    fs::read_to_string(path)
        .expect("missing jsonl file")
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).expect("jsonl line should parse"))
        .collect()
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
        .stdout(predicate::str::contains("execute"))
        .stdout(predicate::str::contains("review"))
        .stdout(predicate::str::contains("status"))
        .stdout(predicate::str::contains("pause"))
        .stdout(predicate::str::contains("resume"))
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
    assert_eq!(task_graph["tasks"].as_array().unwrap().len(), 6);
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
    assert_eq!(task_graph["tasks"][2]["id"], "goal-local-verify");
    assert_eq!(task_graph["tasks"][2]["status"], "pending");
    assert!(task_graph["tasks"][2]["evidence"]
        .as_array()
        .unwrap()
        .is_empty());
    assert_eq!(task_graph["tasks"][3]["id"], "goal-agent-execute");
    assert_eq!(task_graph["tasks"][3]["status"], "pending");
    assert!(task_graph["tasks"][3]["evidence"]
        .as_array()
        .unwrap()
        .is_empty());
    assert_eq!(task_graph["tasks"][4]["id"], "goal-review");
    assert_eq!(task_graph["tasks"][4]["status"], "pending");
    assert_eq!(task_graph["tasks"][5]["id"], "goal-security-review");
    assert_eq!(task_graph["tasks"][5]["status"], "pending");

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
    assert_eq!(proof_json["task_graph_summary"]["total_tasks"], 6);
    assert_eq!(proof_json["task_graph_summary"]["done_tasks"], 2);
    assert_eq!(proof_json["task_graph_summary"]["pending_tasks"], 4);
    assert_eq!(proof_json["post_mutation_gates_ran"], false);
    assert!(proof_json["known_gaps"]
        .as_array()
        .unwrap()
        .iter()
        .any(|gap| gap
            .as_str()
            .unwrap()
            .contains("agent execution has not run")));
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
    assert_eq!(proof_json["post_mutation_gates_ran"], false);
    assert!(proof_json["known_gaps"]
        .as_array()
        .unwrap()
        .iter()
        .any(|gap| gap
            .as_str()
            .unwrap()
            .contains("agent execution has not run")));
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
fn test_goal_execute_runs_mock_wire_agent_and_records_agent_task_evidence() {
    let (_tmp, envs) = isolated_env();
    let project = tempfile::tempdir().expect("temp project");
    write_gate_config(project.path(), "smoke", "echo smoke-ok");

    let mut run = omk_cmd(&envs);
    run.current_dir(project.path())
        .args(["goal", "run", "Execute local verification evidence"])
        .assert()
        .success();

    let mut execute = omk_cmd(&envs);
    execute
        .env("MOCK_KIMI", mock_kimi_path())
        .env("OMK_WIRE_WORKER_POLL_INTERVAL_MS", "50")
        .current_dir(project.path())
        .args(["goal", "execute", "latest"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Execution: not_ready"))
        .stdout(predicate::str::contains("goal-local-verify: done"))
        .stdout(predicate::str::contains("goal-agent-execute: done"))
        .stdout(predicate::str::contains("goal-review: pending"))
        .stdout(predicate::str::contains("goal-security-review: pending"));

    let dirs = goal_dirs(&envs);
    assert_eq!(dirs.len(), 1);
    let task_graph: Value = serde_json::from_str(
        &fs::read_to_string(dirs[0].join("task-graph.json")).expect("missing task graph"),
    )
    .expect("task graph should be JSON");
    assert_eq!(task_graph["tasks"].as_array().unwrap().len(), 6);

    let local_verify = task_graph["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|task| task["id"] == "goal-local-verify")
        .expect("missing goal-local-verify task");
    assert_eq!(local_verify["status"], "done");
    assert_eq!(local_verify["owner_role"], "goal-controller");
    assert!(local_verify["completed_at"].as_str().is_some());
    assert!(local_verify["evidence"]
        .as_array()
        .unwrap()
        .iter()
        .any(|evidence| evidence["path"] == "artifacts/gates"));
    assert!(local_verify["evidence"]
        .as_array()
        .unwrap()
        .iter()
        .any(|evidence| evidence["path"] == "proof.json"));

    let agent_execute = task_graph["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|task| task["id"] == "goal-agent-execute")
        .expect("missing goal-agent-execute task");
    assert_eq!(agent_execute["status"], "done");
    assert_eq!(agent_execute["owner_role"], "executor");
    assert!(agent_execute["completed_at"].as_str().is_some());
    let agent_evidence = agent_execute["evidence"].as_array().unwrap();
    assert!(agent_evidence.iter().any(|evidence| {
        evidence["kind"] == "agent_run"
            && evidence["path"] == "artifacts/agent-runs/goal-agent-execute"
    }));
    assert!(agent_evidence.iter().any(|evidence| {
        evidence["kind"] == "worker_outbox"
            && evidence["path"]
                == "artifacts/agent-runs/goal-agent-execute/workers/goal-agent-worker-0/outbox.jsonl"
    }));
    assert!(agent_evidence.iter().any(|evidence| {
        evidence["kind"] == "wire_events"
            && evidence["path"]
                == "artifacts/agent-runs/goal-agent-execute/workers/goal-agent-worker-0/wire-events.jsonl"
    }));

    assert!(dirs[0]
        .join("artifacts/agent-runs/goal-agent-execute/workers/goal-agent-worker-0/outbox.jsonl")
        .exists());
    assert!(dirs[0]
        .join(
            "artifacts/agent-runs/goal-agent-execute/workers/goal-agent-worker-0/wire-events.jsonl"
        )
        .exists());

    let review = task_graph["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|task| task["id"] == "goal-review")
        .expect("missing goal-review task");
    assert_eq!(review["status"], "pending");
    let security_review = task_graph["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|task| task["id"] == "goal-security-review")
        .expect("missing goal-security-review task");
    assert_eq!(security_review["status"], "pending");

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
    assert_eq!(proof_json["task_graph_summary"]["total_tasks"], 6);
    assert_eq!(proof_json["task_graph_summary"]["done_tasks"], 4);
    assert_eq!(proof_json["task_graph_summary"]["pending_tasks"], 2);
    assert!(!proof_json["known_gaps"]
        .as_array()
        .unwrap()
        .iter()
        .any(|gap| gap
            .as_str()
            .unwrap()
            .contains("agent execution is not implemented")));
    assert!(!proof_json["known_gaps"]
        .as_array()
        .unwrap()
        .iter()
        .any(|gap| gap
            .as_str()
            .unwrap()
            .contains("agent execution has not run")));
    assert!(proof_json["known_gaps"]
        .as_array()
        .unwrap()
        .iter()
        .any(|gap| gap.as_str().unwrap().contains("review evidence")));
}

#[test]
fn test_goal_execute_dispatches_policy_validated_multi_task_agent_wave() {
    let (_tmp, envs) = isolated_env();
    let project = tempfile::tempdir().expect("temp project");
    write_gate_config(project.path(), "smoke", "echo smoke-ok");

    let mut run = omk_cmd(&envs);
    run.current_dir(project.path())
        .args([
            "goal",
            "run",
            "Dispatch a bounded policy-validated controller wave",
            "--max-agents",
            "2",
        ])
        .assert()
        .success();

    let mut execute = omk_cmd(&envs);
    execute
        .env("MOCK_KIMI", mock_kimi_path())
        .env("OMK_WIRE_WORKER_POLL_INTERVAL_MS", "50")
        .current_dir(project.path())
        .args(["goal", "execute", "latest"])
        .assert()
        .success()
        .stdout(predicate::str::contains("goal-agent-execute: done"));

    let dirs = goal_dirs(&envs);
    assert_eq!(dirs.len(), 1);
    let goal_dir = &dirs[0];
    let policy_path = goal_dir.join("artifacts/agent-runs/goal-agent-execute/task-policy.json");
    assert!(policy_path.exists(), "missing task policy artifact");
    let policy: Value = serde_json::from_str(
        &fs::read_to_string(&policy_path).expect("missing task policy artifact"),
    )
    .expect("task policy should be JSON");

    let accepted = policy["accepted_tasks"].as_array().unwrap();
    assert_eq!(
        accepted.len(),
        2,
        "expected two accepted bounded agent tasks: {policy:#?}"
    );
    assert!(accepted.iter().any(|task| {
        task["id"] == "goal-agent-implement"
            && task["budget_secs"].as_u64().unwrap_or_default() > 0
            && task["acceptance"]
                .as_array()
                .unwrap()
                .iter()
                .any(|item| item.as_str().unwrap().contains("bounded project change"))
    }));
    assert!(accepted.iter().any(|task| {
        task["id"] == "goal-agent-verify"
            && task["budget_secs"].as_u64().unwrap_or_default() > 0
            && task["dependencies"]
                .as_array()
                .unwrap()
                .iter()
                .any(|dep| dep == "goal-agent-implement")
    }));

    let rejected = policy["rejected_tasks"].as_array().unwrap();
    assert!(rejected.iter().any(|decision| {
        decision["task"]["id"] == "goal-agent-publish-crates-io"
            && decision["reason"]
                .as_str()
                .unwrap()
                .contains("publishing is disabled")
    }));

    let events = read_jsonl(&goal_dir.join("events.jsonl"));
    assert!(events.iter().any(|event| {
        event["kind"] == "task_proposed" && event["payload"]["task_id"] == "goal-agent-implement"
    }));
    assert!(events.iter().any(|event| {
        event["kind"] == "task_accepted"
            && event["payload"]["task_id"] == "goal-agent-implement"
            && event["payload"]["budget_secs"].as_u64().unwrap_or_default() > 0
    }));
    assert!(events.iter().any(|event| {
        event["kind"] == "task_rejected"
            && event["payload"]["task_id"] == "goal-agent-publish-crates-io"
            && event["payload"]["reason"]
                .as_str()
                .unwrap()
                .contains("publishing is disabled")
    }));

    let outbox_path = goal_dir
        .join("artifacts/agent-runs/goal-agent-execute/workers/goal-agent-worker-0/outbox.jsonl");
    let outbox = read_jsonl(&outbox_path);
    assert!(outbox
        .iter()
        .any(|result| result["task_id"] == "goal-agent-implement"));
    assert!(outbox
        .iter()
        .any(|result| result["task_id"] == "goal-agent-verify"));
    assert!(!outbox
        .iter()
        .any(|result| result["task_id"] == "goal-agent-publish-crates-io"));

    let task_graph: Value = serde_json::from_str(
        &fs::read_to_string(goal_dir.join("task-graph.json")).expect("missing task graph"),
    )
    .expect("task graph should be JSON");
    let agent_execute = task_graph["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|task| task["id"] == "goal-agent-execute")
        .expect("missing goal-agent-execute task");
    let evidence = agent_execute["evidence"].as_array().unwrap();
    assert!(evidence.iter().any(|evidence| {
        evidence["kind"] == "task_policy"
            && evidence["path"] == "artifacts/agent-runs/goal-agent-execute/task-policy.json"
            && evidence["summary"]
                .as_str()
                .unwrap()
                .contains("accepted=2, rejected=1")
    }));
}

#[test]
fn test_goal_execute_accepts_agent_proposed_task_graph_mutation() {
    let (_tmp, envs) = isolated_env();
    let project = tempfile::tempdir().expect("temp project");
    write_gate_config(project.path(), "smoke", "echo smoke-ok");

    let mut run = omk_cmd(&envs);
    run.current_dir(project.path())
        .args(["goal", "run", "Let the agent propose safe follow-up work"])
        .assert()
        .success();

    let proposal = r#"OMK_TASK_PROPOSAL: {"id":"goal-agent-docs-followup","title":"Document follow-up readiness","description":"Document the remaining readiness follow-up found by the agent wave.","dependencies":["goal-agent-execute"],"read_set":["README.md"],"write_set":["README.md"],"risk":"low","acceptance":["README captures the follow-up readiness gap."],"budget_secs":120}"#;
    let mut execute = omk_cmd(&envs);
    execute
        .env("MOCK_KIMI", mock_kimi_path())
        .env("MOCK_KIMI_WIRE_TEXT_WHEN_CONTAINS", "goal-agent-implement")
        .env("MOCK_KIMI_WIRE_TEXT", proposal)
        .env("OMK_WIRE_WORKER_POLL_INTERVAL_MS", "50")
        .current_dir(project.path())
        .args(["goal", "execute", "latest"])
        .assert()
        .success()
        .stdout(predicate::str::contains("goal-agent-execute: done"));

    let dirs = goal_dirs(&envs);
    assert_eq!(dirs.len(), 1);
    let goal_dir = &dirs[0];
    let proposals_path =
        goal_dir.join("artifacts/agent-runs/goal-agent-execute/agent-task-proposals.json");
    assert!(
        proposals_path.exists(),
        "missing agent task proposal artifact"
    );
    let proposals: Value = serde_json::from_str(
        &fs::read_to_string(&proposals_path).expect("missing agent proposal artifact"),
    )
    .expect("agent proposals should be JSON");
    assert!(proposals["accepted_tasks"]
        .as_array()
        .unwrap()
        .iter()
        .any(|task| task["id"] == "goal-agent-docs-followup"));

    let task_graph: Value = serde_json::from_str(
        &fs::read_to_string(goal_dir.join("task-graph.json")).expect("missing task graph"),
    )
    .expect("task graph should be JSON");
    assert_eq!(task_graph["tasks"].as_array().unwrap().len(), 7);
    let followup = task_graph["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|task| task["id"] == "goal-agent-docs-followup")
        .expect("missing accepted agent-proposed task");
    assert_eq!(followup["status"], "pending");
    assert_eq!(followup["owner_role"], "executor");
    assert!(followup["dependencies"]
        .as_array()
        .unwrap()
        .iter()
        .any(|dep| dep == "goal-agent-execute"));

    let events = read_jsonl(&goal_dir.join("events.jsonl"));
    assert!(events.iter().any(|event| {
        event["kind"] == "task_proposed"
            && event["actor"] == "goal-agent-worker-0"
            && event["payload"]["task_id"] == "goal-agent-docs-followup"
    }));
    assert!(events.iter().any(|event| {
        event["kind"] == "task_accepted"
            && event["actor"] == "goal-controller"
            && event["payload"]["task_id"] == "goal-agent-docs-followup"
    }));
    assert!(events.iter().any(|event| {
        event["kind"] == "task_graph_mutated"
            && event["actor"] == "goal-controller"
            && event["payload"]["action"] == "task_added"
            && event["payload"]["source"] == "agent_proposal"
            && event["payload"]["task_id"] == "goal-agent-docs-followup"
            && event["payload"]["task_graph_path"] == "task-graph.json"
            && event["payload"]["proposal_path"]
                == "artifacts/agent-runs/goal-agent-execute/agent-task-proposals.json"
    }));

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
    assert_eq!(proof_json["task_graph_summary"]["total_tasks"], 7);
    assert_eq!(proof_json["task_graph_summary"]["pending_tasks"], 3);
}

#[test]
fn test_goal_execute_rejects_unordered_agent_followup_write_conflict() {
    let (_tmp, envs) = isolated_env();
    let project = tempfile::tempdir().expect("temp project");
    write_gate_config(project.path(), "smoke", "echo smoke-ok");

    let mut run = omk_cmd(&envs);
    run.current_dir(project.path())
        .args(["goal", "run", "Reject unsafe follow-up write conflicts"])
        .assert()
        .success();

    let proposals = concat!(
        r#"OMK_TASK_PROPOSAL: {"id":"goal-agent-docs-followup-a","title":"Document follow-up A","description":"Document the first readiness follow-up.","dependencies":["goal-agent-execute"],"read_set":["README.md"],"write_set":["README.md"],"risk":"low","acceptance":["follow-up A is documented"],"budget_secs":120}"#,
        "\n",
        r#"OMK_TASK_PROPOSAL: {"id":"goal-agent-docs-followup-b","title":"Document follow-up B","description":"Document the second readiness follow-up.","dependencies":["goal-agent-execute"],"read_set":["README.md"],"write_set":["README.md"],"risk":"low","acceptance":["follow-up B is documented"],"budget_secs":120}"#
    );
    let mut execute = omk_cmd(&envs);
    execute
        .env("MOCK_KIMI", mock_kimi_path())
        .env("MOCK_KIMI_WIRE_TEXT_WHEN_CONTAINS", "goal-agent-implement")
        .env("MOCK_KIMI_WIRE_TEXT", proposals)
        .env("OMK_WIRE_WORKER_POLL_INTERVAL_MS", "50")
        .current_dir(project.path())
        .args(["goal", "execute", "latest"])
        .assert()
        .success()
        .stdout(predicate::str::contains("goal-agent-execute: done"));

    let dirs = goal_dirs(&envs);
    assert_eq!(dirs.len(), 1);
    let goal_dir = &dirs[0];
    let proposals_path =
        goal_dir.join("artifacts/agent-runs/goal-agent-execute/agent-task-proposals.json");
    let proposal_policy: Value = serde_json::from_str(
        &fs::read_to_string(&proposals_path).expect("missing agent proposal policy"),
    )
    .expect("agent proposal policy should be JSON");
    assert_eq!(
        proposal_policy["accepted_tasks"].as_array().unwrap().len(),
        1
    );
    assert_eq!(
        proposal_policy["rejected_tasks"].as_array().unwrap().len(),
        1
    );
    assert_eq!(
        proposal_policy["rejected_tasks"][0]["task"]["id"],
        "goal-agent-docs-followup-b"
    );
    assert!(proposal_policy["rejected_tasks"][0]["reason"]
        .as_str()
        .unwrap()
        .contains("write-set conflict with accepted task goal-agent-docs-followup-a: README.md"));

    let task_graph: Value = serde_json::from_str(
        &fs::read_to_string(goal_dir.join("task-graph.json")).expect("missing task graph"),
    )
    .expect("task graph should be JSON");
    assert!(task_graph["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .any(|task| task["id"] == "goal-agent-docs-followup-a"));
    assert!(!task_graph["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .any(|task| task["id"] == "goal-agent-docs-followup-b"));

    let events = read_jsonl(&goal_dir.join("events.jsonl"));
    assert!(events.iter().any(|event| {
        event["kind"] == "task_rejected"
            && event["actor"] == "goal-controller"
            && event["payload"]["task_id"] == "goal-agent-docs-followup-b"
            && event["payload"]["reason"].as_str().unwrap().contains(
                "write-set conflict with accepted task goal-agent-docs-followup-a: README.md",
            )
    }));
}

#[test]
fn test_goal_execute_dispatches_accepted_agent_followup_on_next_execute() {
    let (_tmp, envs) = isolated_env();
    let project = tempfile::tempdir().expect("temp project");
    write_gate_config(project.path(), "smoke", "echo smoke-ok");

    let mut run = omk_cmd(&envs);
    run.current_dir(project.path())
        .args([
            "goal",
            "run",
            "Let the controller continue accepted follow-up work",
        ])
        .assert()
        .success();

    let proposal = r#"OMK_TASK_PROPOSAL: {"id":"goal-agent-docs-followup","title":"Document follow-up readiness","description":"Document the remaining readiness follow-up found by the agent wave.","dependencies":["goal-agent-execute"],"read_set":["README.md"],"write_set":["README.md"],"risk":"low","acceptance":["README captures the follow-up readiness gap."],"budget_secs":120}"#;
    let mut first_execute = omk_cmd(&envs);
    first_execute
        .env("MOCK_KIMI", mock_kimi_path())
        .env("MOCK_KIMI_WIRE_TEXT_WHEN_CONTAINS", "goal-agent-implement")
        .env("MOCK_KIMI_WIRE_TEXT", proposal)
        .env("OMK_WIRE_WORKER_POLL_INTERVAL_MS", "50")
        .current_dir(project.path())
        .args(["goal", "execute", "latest"])
        .assert()
        .success();

    let mut second_execute = omk_cmd(&envs);
    second_execute
        .env("MOCK_KIMI", mock_kimi_path())
        .env("OMK_WIRE_WORKER_POLL_INTERVAL_MS", "50")
        .current_dir(project.path())
        .args(["goal", "execute", "latest"])
        .assert()
        .success();

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
        .expect("missing accepted agent-proposed task");
    assert_eq!(followup["status"], "done");
    assert!(followup["evidence"]
        .as_array()
        .unwrap()
        .iter()
        .any(|evidence| {
            evidence["kind"] == "agent_run"
                && evidence["path"] == "artifacts/agent-runs/goal-agent-followups"
        }));

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
    assert_eq!(proof_json["task_graph_summary"]["total_tasks"], 7);
    assert_eq!(proof_json["task_graph_summary"]["pending_tasks"], 2);
    assert_eq!(proof_json["task_graph_summary"]["done_tasks"], 5);
}

#[test]
fn test_goal_execute_uses_max_agents_worker_pool_for_ready_followups() {
    let (_tmp, envs) = isolated_env();
    let project = tempfile::tempdir().expect("temp project");
    write_gate_config(project.path(), "smoke", "echo smoke-ok");

    let mut run = omk_cmd(&envs);
    run.current_dir(project.path())
        .args([
            "goal",
            "run",
            "Fan out ready follow-up work",
            "--max-agents",
            "2",
        ])
        .assert()
        .success();

    let proposals = concat!(
        r#"OMK_TASK_PROPOSAL: {"id":"goal-agent-docs-followup-a","title":"Document follow-up A","description":"Document the first readiness follow-up.","dependencies":["goal-agent-execute"],"read_set":["README.md"],"write_set":["docs/followup-a.md"],"risk":"low","acceptance":["follow-up A is documented"],"budget_secs":120}"#,
        "\n",
        r#"OMK_TASK_PROPOSAL: {"id":"goal-agent-docs-followup-b","title":"Document follow-up B","description":"Document the second readiness follow-up.","dependencies":["goal-agent-execute"],"read_set":["README.md"],"write_set":["docs/followup-b.md"],"risk":"low","acceptance":["follow-up B is documented"],"budget_secs":120}"#
    );
    let mut first_execute = omk_cmd(&envs);
    first_execute
        .env("MOCK_KIMI", mock_kimi_path())
        .env("MOCK_KIMI_WIRE_TEXT_WHEN_CONTAINS", "goal-agent-implement")
        .env("MOCK_KIMI_WIRE_TEXT", proposals)
        .env("OMK_WIRE_WORKER_POLL_INTERVAL_MS", "50")
        .current_dir(project.path())
        .args(["goal", "execute", "latest"])
        .assert()
        .success();

    let mut second_execute = omk_cmd(&envs);
    second_execute
        .env("MOCK_KIMI", mock_kimi_path())
        .env("OMK_WIRE_WORKER_POLL_INTERVAL_MS", "50")
        .current_dir(project.path())
        .args(["goal", "execute", "latest"])
        .assert()
        .success();

    let dirs = goal_dirs(&envs);
    assert_eq!(dirs.len(), 1);
    let goal_dir = &dirs[0];
    let followup_run = goal_dir.join("artifacts/agent-runs/goal-agent-followups");
    let worker_0_outbox = followup_run.join("workers/goal-agent-worker-0/outbox.jsonl");
    let worker_1_outbox = followup_run.join("workers/goal-agent-worker-1/outbox.jsonl");
    assert!(
        worker_0_outbox.exists(),
        "expected worker 0 outbox for follow-up wave"
    );
    assert!(
        worker_1_outbox.exists(),
        "expected worker 1 outbox when --max-agents 2 allows two ready follow-ups"
    );
    assert!(
        !followup_run.join("workers/goal-agent-worker-2").exists(),
        "worker pool must not exceed --max-agents"
    );

    let mut task_ids: Vec<String> = read_jsonl(&worker_0_outbox)
        .into_iter()
        .chain(read_jsonl(&worker_1_outbox))
        .filter_map(|result| result["task_id"].as_str().map(str::to_string))
        .collect();
    task_ids.sort();
    assert_eq!(
        task_ids,
        vec![
            "goal-agent-docs-followup-a".to_string(),
            "goal-agent-docs-followup-b".to_string(),
        ]
    );

    let policy: Value = serde_json::from_str(
        &fs::read_to_string(followup_run.join("task-policy.json"))
            .expect("missing follow-up task policy"),
    )
    .expect("follow-up task policy should parse");
    assert_eq!(policy["max_agents"], 2);
    assert_eq!(policy["accepted_tasks"].as_array().unwrap().len(), 2);
}

#[test]
fn test_goal_execute_recovers_stale_agent_task_on_another_worker() {
    let (_tmp, envs) = isolated_env();
    let project = tempfile::tempdir().expect("temp project");
    write_gate_config(project.path(), "smoke", "echo smoke-ok");

    let mut run = omk_cmd(&envs);
    run.current_dir(project.path())
        .args([
            "goal",
            "run",
            "Recover stale goal-agent work",
            "--max-agents",
            "2",
        ])
        .assert()
        .success();

    let mut execute = omk_cmd(&envs);
    execute
        .env("MOCK_KIMI", mock_kimi_path())
        .env("MOCK_KIMI_WIRE_STALL_WHEN_CONTAINS", "goal-agent-worker-0")
        .env("OMK_GOAL_AGENT_LEASE_SECS", "1")
        .env("OMK_WIRE_TURN_TIMEOUT_SECS", "30")
        .env("OMK_WIRE_WORKER_POLL_INTERVAL_MS", "50")
        .current_dir(project.path())
        .args(["goal", "execute", "latest"])
        .assert()
        .success()
        .stdout(predicate::str::contains("goal-agent-execute: done"));

    let dirs = goal_dirs(&envs);
    assert_eq!(dirs.len(), 1);
    let goal_dir = &dirs[0];
    let agent_run = goal_dir.join("artifacts/agent-runs/goal-agent-execute");
    let worker_0_outbox = agent_run.join("workers/goal-agent-worker-0/outbox.jsonl");
    let worker_1_outbox = agent_run.join("workers/goal-agent-worker-1/outbox.jsonl");

    let worker_0_results = read_jsonl(&worker_0_outbox);
    assert!(
        !worker_0_results.iter().any(|result| {
            result["task_id"] == "goal-agent-implement" && result["status"] == "success"
        }),
        "stale worker must not successfully complete the recovered task"
    );

    let worker_1_results = read_jsonl(&worker_1_outbox);
    assert!(worker_1_results
        .iter()
        .any(|result| result["task_id"] == "goal-agent-implement"));
    assert!(worker_1_results
        .iter()
        .any(|result| result["task_id"] == "goal-agent-verify"));

    let events = read_jsonl(&goal_dir.join("events.jsonl"));
    assert!(events.iter().any(|event| {
        event["kind"] == "retry_scheduled"
            && event["payload"]["task_id"] == "goal-agent-implement"
            && event["payload"]["reason"] == "stale lease recovered"
            && event["payload"]["stale_worker_id"] == "goal-agent-worker-0"
    }));
    assert!(events.iter().any(|event| {
        event["kind"] == "task_claimed"
            && event["payload"]["task_id"] == "goal-agent-implement"
            && event["payload"]["worker_id"] == "goal-agent-worker-1"
    }));
}

#[test]
fn test_goal_review_records_controller_review_and_security_evidence_after_agent_execution() {
    let (_tmp, envs) = isolated_env();
    let project = tempfile::tempdir().expect("temp project");
    write_gate_config(project.path(), "smoke", "echo smoke-ok");

    let mut run = omk_cmd(&envs);
    run.current_dir(project.path())
        .args(["goal", "run", "Review goal evidence after agent execution"])
        .assert()
        .success();

    let mut execute = omk_cmd(&envs);
    execute
        .env("MOCK_KIMI", mock_kimi_path())
        .env("OMK_WIRE_WORKER_POLL_INTERVAL_MS", "50")
        .current_dir(project.path())
        .args(["goal", "execute", "latest"])
        .assert()
        .success();

    let mut review = omk_cmd(&envs);
    review
        .current_dir(project.path())
        .args(["goal", "review", "latest"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Review: not_ready"))
        .stdout(predicate::str::contains("goal-review: done"))
        .stdout(predicate::str::contains("goal-security-review: done"));

    let dirs = goal_dirs(&envs);
    assert_eq!(dirs.len(), 1);
    let task_graph: Value = serde_json::from_str(
        &fs::read_to_string(dirs[0].join("task-graph.json")).expect("missing task graph"),
    )
    .expect("task graph should be JSON");

    let review_task = task_graph["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|task| task["id"] == "goal-review")
        .expect("missing goal-review task");
    assert_eq!(review_task["status"], "done");
    assert_eq!(review_task["owner_role"], "goal-controller");
    assert!(review_task["completed_at"].as_str().is_some());
    assert!(review_task["evidence"]
        .as_array()
        .unwrap()
        .iter()
        .any(|evidence| evidence["path"] == "artifacts/reviews/goal-review.md"));

    let security_task = task_graph["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|task| task["id"] == "goal-security-review")
        .expect("missing goal-security-review task");
    assert_eq!(security_task["status"], "done");
    assert_eq!(security_task["owner_role"], "goal-controller");
    assert!(security_task["completed_at"].as_str().is_some());
    assert!(security_task["evidence"]
        .as_array()
        .unwrap()
        .iter()
        .any(|evidence| evidence["path"] == "artifacts/reviews/goal-security-review.md"));

    assert!(dirs[0].join("artifacts/reviews/goal-review.md").exists());
    assert!(dirs[0]
        .join("artifacts/reviews/goal-security-review.md")
        .exists());

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
    assert_eq!(proof_json["task_graph_summary"]["total_tasks"], 6);
    assert_eq!(proof_json["task_graph_summary"]["done_tasks"], 6);
    assert_eq!(proof_json["task_graph_summary"]["pending_tasks"], 0);
    assert!(!proof_json["known_gaps"]
        .as_array()
        .unwrap()
        .iter()
        .any(|gap| gap
            .as_str()
            .unwrap()
            .contains("review evidence is not implemented")));
    assert!(!proof_json["known_gaps"]
        .as_array()
        .unwrap()
        .iter()
        .any(|gap| gap
            .as_str()
            .unwrap()
            .contains("security and integration hardening evidence")));
    assert!(proof_json["known_gaps"]
        .as_array()
        .unwrap()
        .iter()
        .any(|gap| gap
            .as_str()
            .unwrap()
            .contains("project mutation and integration loop")));
}

#[test]
fn test_goal_execute_records_agent_mutation_diff_when_worker_changes_project_files() {
    let (_tmp, envs) = isolated_env();
    let project = tempfile::tempdir().expect("temp project");
    write_gate_config(project.path(), "smoke", "echo smoke-ok");
    git(project.path(), &["init"]);
    git(project.path(), &["config", "user.email", "omk@example.com"]);
    git(project.path(), &["config", "user.name", "OMK Test"]);
    git(project.path(), &["add", "."]);
    git(project.path(), &["commit", "-m", "baseline"]);

    let mut run = omk_cmd(&envs);
    run.current_dir(project.path())
        .args(["goal", "run", "Let the agent make a bounded project change"])
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
        .success()
        .stdout(predicate::str::contains("Execution: not_ready"))
        .stdout(predicate::str::contains("goal-agent-execute: done"));

    assert!(project.path().join("agent-output.txt").exists());

    let dirs = goal_dirs(&envs);
    assert_eq!(dirs.len(), 1);
    let task_graph: Value = serde_json::from_str(
        &fs::read_to_string(dirs[0].join("task-graph.json")).expect("missing task graph"),
    )
    .expect("task graph should be JSON");
    let agent_execute = task_graph["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|task| task["id"] == "goal-agent-execute")
        .expect("missing goal-agent-execute task");
    let evidence = agent_execute["evidence"].as_array().unwrap();
    assert!(evidence.iter().any(|evidence| {
        evidence["kind"] == "mutation_diff"
            && evidence["path"] == "artifacts/agent-runs/goal-agent-execute/mutation.diff"
            && evidence["summary"]
                .as_str()
                .unwrap()
                .contains("agent-output.txt")
    }));
    assert!(evidence.iter().any(|evidence| {
        evidence["kind"] == "changed_files"
            && evidence["path"] == "artifacts/agent-runs/goal-agent-execute/changed-files.json"
    }));
    assert!(dirs[0]
        .join("artifacts/agent-runs/goal-agent-execute/mutation.diff")
        .exists());
    assert!(dirs[0]
        .join("artifacts/agent-runs/goal-agent-execute/changed-files.json")
        .exists());

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
    assert!(proof_json["changed_files"]
        .as_array()
        .unwrap()
        .iter()
        .any(|file| file.as_str() == Some("agent-output.txt")));
    assert_eq!(proof_json["post_mutation_gates_ran"], true);
    assert!(!proof_json["known_gaps"]
        .as_array()
        .unwrap()
        .iter()
        .any(|gap| gap
            .as_str()
            .unwrap()
            .contains("verification gates have not rerun after agent execution changes")));
}

#[test]
fn test_goal_execute_reruns_gates_after_agent_mutation() {
    let (_tmp, envs) = isolated_env();
    let project = tempfile::tempdir().expect("temp project");
    let gate_counter = tempfile::NamedTempFile::new().expect("gate counter");
    let script = format!(
        "printf x >> {}; if test -f agent-output.txt; then echo after-agent; else echo before-agent; fi",
        gate_counter.path().display()
    );
    write_gate_config(project.path(), "post-agent-rerun", &script);
    git(project.path(), &["init"]);
    git(project.path(), &["config", "user.email", "omk@example.com"]);
    git(project.path(), &["config", "user.name", "OMK Test"]);
    git(project.path(), &["add", "."]);
    git(project.path(), &["commit", "-m", "baseline"]);

    let mut run = omk_cmd(&envs);
    run.current_dir(project.path())
        .args(["goal", "run", "Rerun gates after an agent mutation"])
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
        .success()
        .stdout(predicate::str::contains("Execution: not_ready"))
        .stdout(predicate::str::contains("goal-agent-execute: done"));

    assert_eq!(
        fs::read_to_string(gate_counter.path()).expect("missing gate counter"),
        "xx"
    );

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
    assert!(proof_json["gates"]
        .as_array()
        .unwrap()
        .iter()
        .any(|gate| gate["name"] == "post-agent-rerun"
            && gate["stdout_summary"].as_str() == Some("after-agent")));
    assert!(!proof_json["known_gaps"]
        .as_array()
        .unwrap()
        .iter()
        .any(|gap| gap
            .as_str()
            .unwrap()
            .contains("verification gates have not rerun after agent execution changes")));
}

#[test]
fn test_goal_execute_blocks_agent_task_when_kimi_is_missing() {
    let (_tmp, envs) = isolated_env();
    let project = tempfile::tempdir().expect("temp project");
    write_gate_config(project.path(), "smoke", "echo smoke-ok");

    let mut run = omk_cmd(&envs);
    run.current_dir(project.path())
        .args(["goal", "run", "Execute without Kimi available"])
        .assert()
        .success();

    let mut execute = omk_cmd(&envs);
    execute
        .env_remove("MOCK_KIMI")
        .env("PATH", "/no-kimi-here")
        .current_dir(project.path())
        .args(["goal", "execute", "latest"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Execution: not_ready"))
        .stdout(predicate::str::contains("goal-local-verify: done"))
        .stdout(predicate::str::contains("goal-agent-execute: blocked"));

    let dirs = goal_dirs(&envs);
    assert_eq!(dirs.len(), 1);
    let task_graph: Value = serde_json::from_str(
        &fs::read_to_string(dirs[0].join("task-graph.json")).expect("missing task graph"),
    )
    .expect("task graph should be JSON");
    let agent_execute = task_graph["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|task| task["id"] == "goal-agent-execute")
        .expect("missing goal-agent-execute task");
    assert_eq!(agent_execute["status"], "blocked");
    assert!(agent_execute["evidence"]
        .as_array()
        .unwrap()
        .iter()
        .any(|evidence| evidence["summary"]
            .as_str()
            .unwrap()
            .contains("Kimi CLI not found")));
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

#[test]
fn test_goal_pause_resume_latest_survives_process_restart() {
    let (_tmp, envs) = isolated_env();

    omk_cmd(&envs)
        .args(["goal", "run", "Pause and resume this durable goal"])
        .assert()
        .success();

    omk_cmd(&envs)
        .args(["goal", "pause", "latest"])
        .assert()
        .success()
        .stdout(predicate::str::contains("paused"))
        .stdout(predicate::str::contains("paused"));

    let paused = omk_cmd(&envs)
        .args(["goal", "show", "latest", "--json"])
        .output()
        .expect("omk goal show failed after pause");
    assert!(
        paused.status.success(),
        "show failed: stdout={} stderr={}",
        String::from_utf8_lossy(&paused.stdout),
        String::from_utf8_lossy(&paused.stderr)
    );
    let paused_json: Value =
        serde_json::from_slice(&paused.stdout).expect("paused show output should be JSON");
    assert_eq!(paused_json["status"], "paused");

    omk_cmd(&envs)
        .args(["goal", "verify", "latest"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("paused"));

    omk_cmd(&envs)
        .args(["goal", "execute", "latest"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("paused"));

    omk_cmd(&envs)
        .args(["goal", "review", "latest"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("paused"));

    omk_cmd(&envs)
        .args(["goal", "resume", "latest"])
        .assert()
        .success()
        .stdout(predicate::str::contains("resumed"))
        .stdout(predicate::str::contains("not_ready"));

    let resumed = omk_cmd(&envs)
        .args(["goal", "show", "latest", "--json"])
        .output()
        .expect("omk goal show failed after resume");
    assert!(
        resumed.status.success(),
        "show failed: stdout={} stderr={}",
        String::from_utf8_lossy(&resumed.stdout),
        String::from_utf8_lossy(&resumed.stderr)
    );
    let resumed_json: Value =
        serde_json::from_slice(&resumed.stdout).expect("resumed show output should be JSON");
    assert_eq!(resumed_json["status"], "not_ready");

    let dirs = goal_dirs(&envs);
    let events = read_jsonl(&dirs[0].join("events.jsonl"));
    assert!(events.iter().any(|event| event["kind"] == "goal_paused"));
    assert!(events.iter().any(|event| event["kind"] == "goal_resumed"));
}
