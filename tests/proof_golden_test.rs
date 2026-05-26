use assert_cmd::Command;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;
use tempfile::TempDir;

mod fixture_runner;
use fixture_runner::FixtureRunner;

const GOAL_PROOF_NOT_READY_SCAFFOLD: &str =
    include_str!("fixtures/goal_proof_not_ready_scaffold.json");
const GOAL_PROOF_VERIFIED_NOT_EXECUTED: &str =
    include_str!("fixtures/goal_proof_verified_not_executed.json");
const GOAL_PROOF_WITH_AGENT_EVIDENCE: &str =
    include_str!("fixtures/goal_proof_with_agent_evidence.json");

#[tokio::test]
async fn test_proof_golden_happy_path() {
    let tmp = TempDir::new().unwrap();
    let runner = FixtureRunner::new(tmp.path(), "golden-happy");

    runner
        .emit_run_started("team", tmp.path(), "Implement feature X")
        .await;
    runner.emit_worker_started("worker-0", "coder").await;
    runner.emit_file_changed("src/feature.rs", "modified").await;
    runner.emit_gate_passed("fmt").await;
    runner.emit_gate_passed("clippy").await;
    runner.emit_gate_passed("test").await;
    runner
        .emit_task_completed("task-1", "worker-0", "Feature implemented")
        .await;
    runner.emit_run_completed().await;

    let proof = runner.generate_proof();
    assert_eq!(proof.status, omk::runtime::proof::ProofStatus::Ready);
    assert_eq!(proof.changed_files.len(), 1);
    assert_eq!(proof.gates.len(), 3);
    assert!(proof.failures.is_empty());
    assert_eq!(proof.readiness(), "ready_for_handoff");
    assert_eq!(
        proof.readiness_text(),
        "Ready for handoff: required gates passed and no blocking failures."
    );
    assert!(proof.summary.contains("ready"));
    let md = proof.to_markdown();
    assert!(md.contains("## Verdict"));
    assert!(md.contains("Readiness verdict: `ready_for_handoff`."));
}

#[tokio::test]
async fn test_proof_golden_with_failures() {
    let tmp = TempDir::new().unwrap();
    let runner = FixtureRunner::new(tmp.path(), "golden-failures");

    runner
        .emit_run_started("team", tmp.path(), "Fix failing tests")
        .await;
    runner.emit_worker_started("worker-0", "coder").await;
    runner.emit_file_changed("src/lib.rs", "modified").await;
    runner.emit_gate_passed("fmt").await;
    runner.emit_gate_failed("test", "3 tests failed").await;
    runner
        .emit_task_completed("task-1", "worker-0", "Partial fix")
        .await;
    runner.emit_run_completed().await;

    let proof = runner.generate_proof();
    // Required gate failure results in Failed status.
    assert_eq!(proof.status, omk::runtime::proof::ProofStatus::Failed);
    assert_eq!(proof.gates.len(), 2); // fmt + test
    assert_eq!(proof.failures.len(), 1);
    assert_eq!(proof.readiness(), "blocked");
    assert_eq!(
        proof.readiness_text(),
        "Blocked: failures or required gate failures must be resolved."
    );
    assert!(proof.known_gaps.contains(&"gate test failed".to_string()));
    let md = proof.to_markdown();
    assert!(md.contains("Readiness verdict: `blocked`."));
}

#[tokio::test]
async fn test_proof_golden_empty_run() {
    let tmp = TempDir::new().unwrap();
    let runner = FixtureRunner::new(tmp.path(), "golden-empty");

    runner
        .emit_run_started("team", tmp.path(), "Refactor")
        .await;
    runner.emit_run_completed().await;

    let proof = runner.generate_proof();
    assert_eq!(proof.status, omk::runtime::proof::ProofStatus::NotReady);
    assert_eq!(proof.readiness(), "needs_follow_up");
    assert_eq!(
        proof.readiness_text(),
        "Needs follow-up: required gates are incomplete or missing."
    );
    assert!(proof.changed_files.is_empty());
    assert!(proof.gates.is_empty());
}

#[tokio::test]
async fn test_proof_from_gate_results_direct() {
    use omk::runtime::gates::GateResult;
    use omk::runtime::proof::{ProofGenerator, ProofStatus};

    let gates = vec![
        GateResult {
            name: "fmt".to_string(),
            passed: true,
            stdout: "ok".to_string(),
            stderr: String::new(),
            duration_ms: 100,
            required: true,
            command_line: "cargo fmt --check".to_string(),
            exit_code: Some(0),
            timed_out: false,
            stdout_summary: Some("ok".to_string()),
            stderr_summary: Some(String::new()),
            output_path: Some("/tmp/gates/fmt.log".to_string()),
            timeout_secs: 120,
            circuit_breaker_open: false,
        },
        GateResult {
            name: "clippy".to_string(),
            passed: true,
            stdout: "ok".to_string(),
            stderr: String::new(),
            duration_ms: 200,
            required: true,
            command_line: "cargo clippy -- -D warnings".to_string(),
            exit_code: Some(0),
            timed_out: false,
            stdout_summary: Some("ok".to_string()),
            stderr_summary: Some(String::new()),
            output_path: Some("/tmp/gates/clippy.log".to_string()),
            timeout_secs: 120,
            circuit_breaker_open: false,
        },
        GateResult {
            name: "test".to_string(),
            passed: false,
            stdout: String::new(),
            stderr: "1 test failed".to_string(),
            duration_ms: 500,
            required: true,
            command_line: "cargo test".to_string(),
            exit_code: Some(101),
            timed_out: false,
            stdout_summary: Some(String::new()),
            stderr_summary: Some("1 test failed".to_string()),
            output_path: Some("/tmp/gates/test.log".to_string()),
            timeout_secs: 120,
            circuit_breaker_open: false,
        },
    ];

    let proof = ProofGenerator::from_gate_results(
        omk::runtime::events::RunId("direct-run".to_string()),
        &gates,
        &["src/lib.rs".to_string()],
        &[],
    );
    // Required gate failure results in Failed status.
    assert_eq!(proof.status, ProofStatus::Failed);
    assert_eq!(proof.gates.len(), 3);
    assert_eq!(proof.changed_files.len(), 1);
    assert!(
        proof.gates[0]
            .evidence
            .as_ref()
            .and_then(|e| e.get("command_line"))
            .and_then(|v| v.as_str())
            .map(|v| v.contains("cargo fmt --check"))
            .unwrap_or(false),
        "proof gate evidence should include command line"
    );
}

#[test]
fn test_goal_proof_golden_not_ready_scaffold() {
    let (_tmp, envs) = isolated_env();
    let project = TempDir::new().unwrap();

    let mut run = omk_cmd(&envs);
    run.current_dir(project.path())
        .args([
            "goal",
            "run",
            "Build deterministic proof snapshot coverage",
            "--until-ready",
        ])
        .assert()
        .success();

    let proof = goal_proof_json(&envs, project.path());
    assert_goal_proof_fixture(
        proof,
        "tests/fixtures/goal_proof_not_ready_scaffold.json",
        GOAL_PROOF_NOT_READY_SCAFFOLD,
    );
}

#[test]
fn test_goal_proof_golden_verified_but_not_executed() {
    let (_tmp, envs) = isolated_env();
    let project = TempDir::new().unwrap();
    write_gate_config(project.path(), "smoke", "echo smoke-ok");

    let mut run = omk_cmd(&envs);
    run.current_dir(project.path())
        .args(["goal", "run", "Verify deterministic goal proof output"])
        .assert()
        .success();

    let mut verify = omk_cmd(&envs);
    verify
        .current_dir(project.path())
        .args(["goal", "verify", "latest"])
        .assert()
        .success();

    let proof = goal_proof_json(&envs, project.path());
    assert_goal_proof_fixture(
        proof,
        "tests/fixtures/goal_proof_verified_not_executed.json",
        GOAL_PROOF_VERIFIED_NOT_EXECUTED,
    );
}

#[test]
fn test_goal_proof_golden_with_agent_evidence() {
    let (_tmp, envs) = isolated_env();
    let project = TempDir::new().unwrap();
    write_gate_config(project.path(), "smoke", "echo smoke-ok");
    git(project.path(), &["init"]);
    git(project.path(), &["checkout", "-b", "proof-branch"]);
    git(project.path(), &["config", "user.email", "omk@example.com"]);
    git(project.path(), &["config", "user.name", "OMK Test"]);
    git(project.path(), &["add", ".omk/gates.toml"]);
    git(project.path(), &["commit", "-m", "baseline"]);

    let mut run = omk_cmd(&envs);
    run.current_dir(project.path())
        .args(["goal", "run", "Capture agent evidence in goal proof output"])
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

    let proof = goal_proof_json(&envs, project.path());
    assert_goal_proof_fixture(
        proof,
        "tests/fixtures/goal_proof_with_agent_evidence.json",
        GOAL_PROOF_WITH_AGENT_EVIDENCE,
    );
}

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

fn write_gate_config(project_dir: &Path, gate_name: &str, script: &str) {
    let omk_dir = project_dir.join(".omk");
    fs::create_dir_all(&omk_dir).unwrap();
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
    .unwrap();
}

fn git(project_dir: &Path, args: &[&str]) {
    let output = StdCommand::new("git")
        .arg("-C")
        .arg(project_dir)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {:?} failed: stdout={} stderr={}",
        args,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn goal_proof_json(envs: &[(&'static str, PathBuf)], project_dir: &Path) -> Value {
    let mut cmd = omk_cmd(envs);
    let output = cmd
        .current_dir(project_dir)
        .args(["goal", "proof", "latest", "--json"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "goal proof failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn assert_goal_proof_fixture(actual: Value, fixture_path: &str, expected: &str) {
    let actual = normalize_goal_proof(actual);
    let actual = format!("{}\n", serde_json::to_string_pretty(&actual).unwrap());
    if expected.trim().is_empty() {
        panic!("fixture {fixture_path} is empty; update it with:\n{actual}");
    }
    assert_eq!(
        actual, expected,
        "goal proof fixture drifted: {fixture_path}"
    );
}

fn normalize_goal_proof(mut value: Value) -> Value {
    value["goal_id"] = json!("<goal-id>");
    value["generated_at"] = json!("<timestamp>");

    if let Some(artifacts) = value.get_mut("artifacts").and_then(Value::as_array_mut) {
        for artifact in artifacts {
            artifact["created_at"] = json!("<timestamp>");
        }
    }

    if let Some(gates) = value.get_mut("gates").and_then(Value::as_array_mut) {
        for gate in gates {
            gate["duration_ms"] = json!(0);
            if gate.get("output_path").is_some() {
                gate["output_path"] = json!("<gate-output-path>");
            }
        }
    }

    if let Some(commits) = value.get_mut("commits").and_then(Value::as_array_mut) {
        for commit in commits {
            *commit = json!("<git-head>");
        }
    }

    if let Some(git) = value.get_mut("git").and_then(Value::as_object_mut) {
        git.insert("head".to_string(), json!("<git-head>"));
    }

    value
}
