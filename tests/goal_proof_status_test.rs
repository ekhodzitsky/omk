use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
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

fn goal_id(goal_dir: &Path) -> String {
    let state: Value =
        serde_json::from_slice(&fs::read(goal_dir.join("goal.json")).expect("missing goal.json"))
            .expect("goal state should parse");
    state["goal_id"]
        .as_str()
        .expect("goal_id should be a string")
        .to_string()
}

fn goal_proof_json(envs: &[(&'static str, PathBuf)], goal_id: &str, project_dir: &Path) -> Value {
    let output = omk_cmd(envs)
        .current_dir(project_dir)
        .args(["goal", "proof", goal_id, "--json"])
        .output()
        .expect("omk goal proof failed to start");
    assert!(
        output.status.success(),
        "goal proof failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("proof output should parse")
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

#[test]
fn proof_names_blocked_on_human_oracle_gap() {
    let (_tmp, envs) = isolated_env();
    let project = TempDir::new().expect("temp project");

    omk_cmd(&envs)
        .current_dir(project.path())
        .args(["goal", "run", "Make it awesome", "--until-ready"])
        .assert()
        .success()
        .stdout(predicate::str::contains("blocked_on_human"));

    let dirs = goal_dirs(&envs);
    let proof = goal_proof_json(&envs, &goal_id(&dirs[0]), project.path());
    assert_eq!(proof["status"], "blocked_on_human");
    assert!(proof["readiness"]
        .as_str()
        .is_some_and(|value| value.contains("blocked on human")));
    assert!(proof["known_gaps"]
        .as_array()
        .is_some_and(|gaps| gaps.iter().any(|gap| gap
            .as_str()
            .is_some_and(|value| value.contains("goal oracle is not testable")))));
    assert!(proof["human_decisions_required"]
        .as_array()
        .is_some_and(|decisions| decisions.iter().any(|decision| decision
            .as_str()
            .is_some_and(|value| value.contains("testable success criteria")))));
}

#[test]
fn proof_reflects_cancelled_state_after_operator_cancel() {
    let (_tmp, envs) = isolated_env();
    let project = TempDir::new().expect("temp project");

    omk_cmd(&envs)
        .current_dir(project.path())
        .args(["goal", "run", "Cancel proof status after operator request"])
        .assert()
        .success();
    omk_cmd(&envs)
        .current_dir(project.path())
        .args(["goal", "cancel", "latest"])
        .assert()
        .success();

    let dirs = goal_dirs(&envs);
    let proof = goal_proof_json(&envs, &goal_id(&dirs[0]), project.path());
    assert_eq!(proof["status"], "cancelled");
    assert!(proof["readiness"]
        .as_str()
        .is_some_and(|value| value.contains("cancelled")));
    assert!(proof["known_gaps"]
        .as_array()
        .is_some_and(|gaps| gaps.iter().any(|gap| gap
            .as_str()
            .is_some_and(|value| value.contains("cancelled by user")))));
}

#[test]
fn proof_reflects_needs_more_budget_after_budget_guard() {
    let (tmp, envs) = isolated_env();
    let project = tmp.path().join("project");
    fs::create_dir_all(&project).expect("failed to create project dir");
    write_gate_config(&project, "should-not-run", "touch should-not-run");

    omk_cmd(&envs)
        .current_dir(&project)
        .args([
            "goal",
            "run",
            "Record proof status after budget exhaustion",
            "--budget-time",
            "0s",
        ])
        .assert()
        .success();
    omk_cmd(&envs)
        .current_dir(&project)
        .args(["goal", "verify", "latest"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("needs more budget"));

    let dirs = goal_dirs(&envs);
    let proof = goal_proof_json(&envs, &goal_id(&dirs[0]), &project);
    assert_eq!(proof["status"], "needs_more_budget");
    assert!(proof["readiness"]
        .as_str()
        .is_some_and(|value| value.contains("needs more budget")));
    assert!(proof["known_gaps"].as_array().is_some_and(|gaps| gaps
        .iter()
        .any(|gap| gap.as_str().is_some_and(|value| value.contains("budget")))));
}

#[test]
fn proof_rebuild_preserves_failed_infra_like_state() {
    let (_tmp, envs) = isolated_env();
    let project = TempDir::new().expect("temp project");

    for (status, reason) in [
        (
            "failed_infra",
            "worker process exited before producing evidence",
        ),
        (
            "blocked_on_external",
            "external service credentials are missing",
        ),
    ] {
        omk_cmd(&envs)
            .current_dir(project.path())
            .args([
                "goal",
                "run",
                "Preserve terminal proof status during recovery",
            ])
            .assert()
            .success();

        let dirs = goal_dirs(&envs);
        let goal_dir = dirs.last().expect("missing goal dir");
        let state_path = goal_dir.join("goal.json");
        let mut state: Value =
            serde_json::from_slice(&fs::read(&state_path).expect("missing goal.json"))
                .expect("goal state should parse");
        state["status"] = json!(status);
        state["failure"] = json!({
            "reason": reason,
            "recorded_at": state["updated_at"].clone(),
        });
        fs::write(
            &state_path,
            serde_json::to_vec_pretty(&state).expect("state should serialize"),
        )
        .expect("failed to write goal state");
        fs::remove_file(goal_dir.join("proof.json")).expect("failed to remove proof");

        let proof = goal_proof_json(&envs, state["goal_id"].as_str().unwrap(), project.path());
        assert_eq!(proof["status"], status);
        assert!(proof["readiness"]
            .as_str()
            .is_some_and(|value| value.contains(reason)));
        assert!(proof["known_gaps"].as_array().is_some_and(|gaps| gaps
            .iter()
            .any(|gap| gap.as_str().is_some_and(|value| value.contains(reason)))));
    }
}
