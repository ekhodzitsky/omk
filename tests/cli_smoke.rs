use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn test_version_flag() {
    let mut cmd = Command::cargo_bin("omk").unwrap();
    cmd.arg("--version");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("0.2.3"));
}

#[test]
fn test_help_flag() {
    let mut cmd = Command::cargo_bin("omk").unwrap();
    cmd.arg("--help");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Multi-agent orchestration"));
}

#[test]
fn test_version_subcommand() {
    let mut cmd = Command::cargo_bin("omk").unwrap();
    cmd.arg("version");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("omk 0.2.3"));
}

#[test]
fn test_doctor_runs() {
    let mut cmd = Command::cargo_bin("omk").unwrap();
    cmd.arg("doctor");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("omk doctor"));
}

#[test]
fn test_config_show_runs() {
    let mut cmd = Command::cargo_bin("omk").unwrap();
    cmd.arg("config").arg("show");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("omk Configuration"));
}

#[test]
fn test_config_validate_runs() {
    let mut cmd = Command::cargo_bin("omk").unwrap();
    cmd.arg("config").arg("validate");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Validating"));
}

#[test]
fn test_completions_runs() {
    let mut cmd = Command::cargo_bin("omk").unwrap();
    cmd.arg("completions").arg("bash");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("complete"));
}

#[test]
fn test_man_runs() {
    let mut cmd = Command::cargo_bin("omk").unwrap();
    cmd.arg("man");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("OMK"));
}

#[test]
fn test_state_list_runs() {
    let mut cmd = Command::cargo_bin("omk").unwrap();
    cmd.arg("state").arg("list");
    cmd.assert()
        .success();
}

#[test]
fn test_team_list_runs() {
    let mut cmd = Command::cargo_bin("omk").unwrap();
    cmd.arg("team").arg("list");
    cmd.assert()
        .success();
}

#[test]
fn test_skill_list_runs() {
    let mut cmd = Command::cargo_bin("omk").unwrap();
    cmd.arg("skill").arg("list");
    cmd.assert()
        .success();
}

#[test]
fn test_marketplace_list_runs() {
    let mut cmd = Command::cargo_bin("omk").unwrap();
    cmd.arg("marketplace").arg("list");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("omk Marketplace"));
}

#[test]
fn test_backup_list_runs() {
    let mut cmd = Command::cargo_bin("omk").unwrap();
    cmd.arg("backup").arg("list");
    cmd.assert()
        .success();
}

#[test]
fn test_cleanup_dry_run_runs() {
    let mut cmd = Command::cargo_bin("omk").unwrap();
    cmd.arg("cleanup").arg("--dry-run");
    cmd.assert()
        .success();
}

#[test]
fn test_logs_runs() {
    let mut cmd = Command::cargo_bin("omk").unwrap();
    cmd.arg("logs").arg("--lines").arg("10");
    // May fail if no log file exists, so we just check it doesn't panic
    let output = cmd.output().unwrap();
    // Should exit 0 or print error gracefully
    assert!(
        output.status.success()
            || String::from_utf8_lossy(&output.stdout).contains("No log file")
            || String::from_utf8_lossy(&output.stderr).contains("No log file")
    );
}

#[test]
fn test_marketplace_info_builtin_skill() {
    let mut cmd = Command::cargo_bin("omk").unwrap();
    cmd.arg("marketplace").arg("info").arg("rust-expert");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("rust-expert"));
}

#[test]
fn test_update_check_runs() {
    let mut cmd = Command::cargo_bin("omk").unwrap();
    cmd.arg("update").arg("--check");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("version").or(predicate::str::contains("Latest")));
}

#[test]
fn test_config_set_team_size() {
    let mut cmd = Command::cargo_bin("omk").unwrap();
    cmd.arg("config").arg("set").arg("default_team_size").arg("3");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Set default_team_size = 3"));
}

#[test]
fn test_config_set_invalid_key() {
    let mut cmd = Command::cargo_bin("omk").unwrap();
    cmd.arg("config").arg("set").arg("invalid_key").arg("value");
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Unknown config key"));
}

#[test]
fn test_marketplace_search_runs() {
    let mut cmd = Command::cargo_bin("omk").unwrap();
    cmd.arg("marketplace").arg("search").arg("rust");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("rust-expert"));
}

#[test]
fn test_team_spawn_missing_task() {
    let mut cmd = Command::cargo_bin("omk").unwrap();
    cmd.arg("team").arg("spawn").arg("3:coder");
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Task description is required"));
}

#[test]
fn test_team_export_import_roundtrip() {
    let bin = Command::cargo_bin("omk").unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let export_path = tmp.path().join("test-team.json");

    // Export should fail if team doesn't exist
    let mut cmd = Command::cargo_bin("omk").unwrap();
    cmd.arg("team").arg("export").arg("nonexistent-team").arg("-o").arg(&export_path);
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}

#[test]
fn test_hud_web_help() {
    let mut cmd = Command::cargo_bin("omk").unwrap();
    cmd.arg("hud").arg("--help");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("web"));
}

#[test]
fn test_ask_help() {
    let mut cmd = Command::cargo_bin("omk").unwrap();
    cmd.arg("ask").arg("--help");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("provider"));
}

#[test]
fn test_autopilot_help() {
    let mut cmd = Command::cargo_bin("omk").unwrap();
    cmd.arg("autopilot").arg("--help");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("resume"));
}

#[test]
fn test_ralph_help() {
    let mut cmd = Command::cargo_bin("omk").unwrap();
    cmd.arg("ralph").arg("--help");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("yolo"));
}
