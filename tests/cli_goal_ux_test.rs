//! CLI UX coverage for `omk goal`:
//! help text, validation, and actionable error messages.

use assert_cmd::Command;
use predicates::prelude::*;
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

// ---------- help text ----------

#[test]
fn test_goal_top_help_describes_runtime_and_lists_examples() {
    let mut cmd = Command::cargo_bin("omk").unwrap();
    cmd.arg("goal").arg("--help");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Goal runtime"))
        .stdout(predicate::str::contains("Examples:"))
        .stdout(predicate::str::contains("omk goal run"))
        .stdout(predicate::str::contains("omk goal show latest --json"))
        .stdout(predicate::str::contains("Goal state is stored"));
}

#[test]
fn test_goal_run_help_lists_examples_and_documents_budget_units() {
    let mut cmd = Command::cargo_bin("omk").unwrap();
    cmd.arg("goal").arg("run").arg("--help");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Examples:"))
        .stdout(predicate::str::contains("Fix all failing cargo tests"))
        .stdout(predicate::str::contains("suffix s/m/h/d"))
        .stdout(predicate::str::contains("must be > 0"));
}

#[test]
fn test_goal_budget_add_help_documents_required_flag() {
    let mut cmd = Command::cargo_bin("omk").unwrap();
    cmd.arg("goal").arg("budget-add").arg("--help");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Examples:"))
        .stdout(predicate::str::contains(
            "At least one of --time / --tokens / --usd must be provided.",
        ));
}

#[test]
fn test_goal_show_help_documents_json_shortcut_and_format() {
    let mut cmd = Command::cargo_bin("omk").unwrap();
    cmd.arg("goal").arg("show").arg("--help");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("--format"))
        .stdout(predicate::str::contains("--json"))
        .stdout(predicate::str::contains("Shortcut for"));
}

#[test]
fn test_goal_list_status_show_help_are_present() {
    for sub in ["list", "status", "show", "proof", "replay", "budget"] {
        let mut cmd = Command::cargo_bin("omk").unwrap();
        cmd.arg("goal").arg(sub).arg("--help");
        cmd.assert()
            .success()
            .stdout(predicate::str::contains("Examples:"));
    }
}

// ---------- input validation ----------

#[test]
fn test_goal_run_rejects_empty_goal_with_actionable_error() {
    let (_tmp, envs) = isolated_env();
    omk_cmd(&envs)
        .args(["goal", "run", ""])
        .assert()
        .failure()
        .stderr(predicate::str::contains("goal text cannot be empty"))
        .stderr(predicate::str::contains("omk goal run \""));
}

#[test]
fn test_goal_run_rejects_whitespace_only_goal() {
    let (_tmp, envs) = isolated_env();
    omk_cmd(&envs)
        .args(["goal", "run", "   "])
        .assert()
        .failure()
        .stderr(predicate::str::contains("goal text cannot be empty"));
}

#[test]
fn test_goal_run_rejects_invalid_budget_time_format() {
    let (_tmp, envs) = isolated_env();
    omk_cmd(&envs)
        .args(["goal", "run", "Fix tests", "--budget-time", "nope"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid --budget-time"))
        .stderr(predicate::str::contains("invalid duration 'nope'"))
        .stderr(predicate::str::contains("s/m/h/d"));
}

#[test]
fn test_goal_run_rejects_zero_budget_tokens() {
    let (_tmp, envs) = isolated_env();
    omk_cmd(&envs)
        .args(["goal", "run", "Fix tests", "--budget-tokens", "0"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "--budget-tokens must be greater than zero",
        ));
}

#[test]
fn test_goal_run_rejects_zero_budget_usd() {
    let (_tmp, envs) = isolated_env();
    omk_cmd(&envs)
        .args(["goal", "run", "Fix tests", "--budget-usd", "0"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "--budget-usd must be a positive, finite number",
        ));
}

#[test]
fn test_goal_run_rejects_zero_max_agents() {
    let (_tmp, envs) = isolated_env();
    omk_cmd(&envs)
        .args(["goal", "run", "Fix tests", "--max-agents", "0"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "--max-agents must be greater than zero",
        ));
}

#[test]
fn test_goal_budget_add_requires_at_least_one_flag() {
    let (_tmp, envs) = isolated_env();
    omk_cmd(&envs)
        .args(["goal", "budget-add", "latest"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "Provide at least one budget extension",
        ))
        .stderr(predicate::str::contains("--time"))
        .stderr(predicate::str::contains("--tokens"))
        .stderr(predicate::str::contains("--usd"));
}

#[test]
fn test_goal_budget_add_rejects_invalid_time_eagerly() {
    let (_tmp, envs) = isolated_env();
    // No goal exists yet, but validation should happen before resolve.
    omk_cmd(&envs)
        .args(["goal", "budget-add", "latest", "--time", "abc"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid duration 'abc'"));
}

// ---------- resolve errors ----------

#[test]
fn test_goal_show_latest_when_no_goals_emits_actionable_hint() {
    let (_tmp, envs) = isolated_env();
    omk_cmd(&envs)
        .args(["goal", "show", "latest"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("No goals found"))
        .stderr(predicate::str::contains("omk goal run"));
}

#[test]
fn test_goal_status_with_unknown_id_hints_at_list() {
    let (_tmp, envs) = isolated_env();
    omk_cmd(&envs)
        .args(["goal", "status", "goal-does-not-exist"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "Goal 'goal-does-not-exist' not found",
        ))
        .stderr(predicate::str::contains("omk goal list"));
}

#[test]
fn test_goal_proof_with_unknown_id_hints_at_list() {
    let (_tmp, envs) = isolated_env();
    omk_cmd(&envs)
        .args(["goal", "proof", "missing-9999"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Goal 'missing-9999' not found"))
        .stderr(predicate::str::contains("omk goal list"));
}

#[test]
fn test_goal_list_empty_state_is_user_friendly() {
    let (_tmp, envs) = isolated_env();
    omk_cmd(&envs)
        .args(["goal", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No goals found"))
        .stdout(predicate::str::contains("omk goal run"));
}

// ---------- output consistency ----------

#[test]
fn test_goal_show_json_shortcut_matches_format_json() {
    let (_tmp, envs) = isolated_env();
    omk_cmd(&envs)
        .args(["goal", "run", "Ship CLI UX polish for omk goal"])
        .assert()
        .success();

    let json_via_flag = omk_cmd(&envs)
        .args(["goal", "show", "latest", "--json"])
        .output()
        .expect("--json shortcut should succeed");
    assert!(
        json_via_flag.status.success(),
        "--json shortcut failed: stderr={}",
        String::from_utf8_lossy(&json_via_flag.stderr)
    );
    let parsed_shortcut: serde_json::Value =
        serde_json::from_slice(&json_via_flag.stdout).expect("--json output should parse as JSON");

    let json_via_format = omk_cmd(&envs)
        .args(["goal", "show", "latest", "--format", "json"])
        .output()
        .expect("--format json should succeed");
    assert!(
        json_via_format.status.success(),
        "--format json failed: stderr={}",
        String::from_utf8_lossy(&json_via_format.stderr)
    );
    let parsed_format: serde_json::Value = serde_json::from_slice(&json_via_format.stdout)
        .expect("--format json output should parse as JSON");

    assert_eq!(parsed_shortcut, parsed_format);
}

#[test]
fn test_goal_show_rejects_conflicting_json_and_format_flags() {
    let (_tmp, envs) = isolated_env();
    omk_cmd(&envs)
        .args(["goal", "show", "latest", "--json", "--format", "text"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with"));
}

#[test]
fn test_goal_run_success_lists_next_steps_for_actionable_goal() {
    let (_tmp, envs) = isolated_env();
    omk_cmd(&envs)
        .args([
            "goal",
            "run",
            "Fix all failing cargo tests in src/runtime/goal",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Goal scaffold created"))
        .stdout(predicate::str::contains("Next steps:"))
        .stdout(predicate::str::contains("omk goal show latest"))
        .stdout(predicate::str::contains("omk goal verify latest"))
        .stdout(predicate::str::contains("omk goal execute latest"));
}

#[test]
fn test_goal_pause_includes_resume_hint() {
    let (_tmp, envs) = isolated_env();
    omk_cmd(&envs)
        .args([
            "goal",
            "run",
            "Fix all failing cargo tests in src/runtime/goal",
        ])
        .assert()
        .success();
    omk_cmd(&envs)
        .args(["goal", "pause", "latest"])
        .assert()
        .success()
        .stdout(predicate::str::contains("paused"))
        .stdout(predicate::str::contains("omk goal resume"));
}
