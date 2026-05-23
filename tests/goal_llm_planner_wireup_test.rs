use assert_cmd::Command;
use predicates::prelude::*;
use std::path::PathBuf;

fn omk_cmd() -> Command {
    Command::cargo_bin("omk").unwrap()
}

fn isolated_env() -> (tempfile::TempDir, Vec<(&'static str, PathBuf)>) {
    omk::test_helpers::isolated_xdg_env()
}

#[test]
fn test_help_shows_new_flags() {
    let mut cmd = omk_cmd();
    cmd.args(["goal", "run", "--help"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("--no-llm-planner"))
        .stdout(predicate::str::contains("--planner-token-budget"))
        .stdout(predicate::str::contains("Default: 8000"));
}

#[test]
fn test_no_llm_planner_flag_runs_successfully() {
    let (_tmp, envs) = isolated_env();
    let project = tempfile::tempdir().expect("temp project");

    let mut cmd = omk_cmd();
    for (key, value) in &envs {
        cmd.env(key, value);
    }
    cmd.current_dir(project.path()).args([
        "goal",
        "run",
        "Test goal with no LLM planner",
        "--no-llm-planner",
    ]);

    cmd.assert().success().stderr(predicate::str::contains(
        "goal: using stub planner (--no-llm-planner)",
    ));
}

#[test]
fn test_planner_token_budget_custom_accepted() {
    let (_tmp, envs) = isolated_env();
    let project = tempfile::tempdir().expect("temp project");

    let mut cmd = omk_cmd();
    for (key, value) in &envs {
        cmd.env(key, value);
    }
    cmd.current_dir(project.path()).args([
        "goal",
        "run",
        "Test goal with custom budget",
        "--planner-token-budget",
        "4000",
    ]);

    // Should succeed (fallback to stub if kimi unavailable, or use LLM if available)
    cmd.assert().success();
}

#[test]
fn test_fallback_when_kimi_unavailable() {
    let (_tmp, envs) = isolated_env();
    let project = tempfile::tempdir().expect("temp project");

    let mut cmd = omk_cmd();
    for (key, value) in &envs {
        cmd.env(key, value);
    }
    cmd.env_remove("MOCK_KIMI")
        .env("PATH", "/no-kimi-here")
        .current_dir(project.path())
        .args(["goal", "run", "Test goal without kimi"]);

    cmd.assert()
        .success()
        .stderr(predicate::str::contains("goal: llm planner unavailable"));
}

#[test]
fn test_existing_args_with_new_flags() {
    let (_tmp, envs) = isolated_env();
    let project = tempfile::tempdir().expect("temp project");

    let mut cmd = omk_cmd();
    for (key, value) in &envs {
        cmd.env(key, value);
    }
    cmd.current_dir(project.path()).args([
        "goal",
        "run",
        "Test goal with mixed args",
        "--until-ready",
        "--budget-time",
        "1h",
        "--max-agents",
        "2",
        "--no-llm-planner",
        "--planner-token-budget",
        "4000",
    ]);

    cmd.assert().success();
}
