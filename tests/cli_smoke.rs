use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;

fn isolated_env() -> (tempfile::TempDir, Vec<(&'static str, std::path::PathBuf)>) {
    omk::test_helpers::isolated_xdg_env()
}

#[test]
fn test_version_flag() {
    let mut cmd = Command::cargo_bin("omk").unwrap();
    cmd.arg("--version");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn test_help_flag() {
    let mut cmd = Command::cargo_bin("omk").unwrap();
    cmd.arg("--help");
    cmd.assert().success().stdout(predicate::str::contains(
        "Scheduler-backed team orchestration and Kimi asset tooling",
    ));
}

#[test]
fn test_help_honesty_top_level_team_and_kimi() {
    let mut cmd = Command::cargo_bin("omk").unwrap();
    cmd.arg("--help");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains(
            "team         Team orchestration (Wire scheduler runtime)",
        ))
        .stdout(predicate::str::contains("tmux").not())
        .stdout(predicate::str::contains(
            "kimi         Kimi asset commands (sync/install/doctor + listing/rollback surfaces)",
        ));
}

#[test]
fn test_help_honesty_team_surfaces() {
    let mut team_cmd = Command::cargo_bin("omk").unwrap();
    team_cmd.arg("team").arg("--help");
    team_cmd
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Team orchestration (Wire scheduler runtime)",
        ))
        .stdout(predicate::str::contains(
            "run       Run a scheduler-backed team workflow",
        ))
        .stdout(predicate::str::contains("spawn").not())
        .stdout(predicate::str::contains("attach").not())
        .stdout(predicate::str::contains("broadcast").not())
        .stdout(predicate::str::contains("tmux").not());
}

#[test]
fn test_help_honesty_kimi_surfaces() {
    let mut kimi_cmd = Command::cargo_bin("omk").unwrap();
    kimi_cmd.arg("kimi").arg("--help");
    kimi_cmd
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Kimi asset commands (sync/install/doctor + listing/rollback surfaces)",
        ))
        .stdout(predicate::str::contains(
            "sync      Sync OMK assets for current Kimi surfaces (project + user scope)",
        ))
        .stdout(predicate::str::contains(
            "agents    List bundled OMK role agent templates",
        ))
        .stdout(predicate::str::contains(
            "hooks     List bundled OMK project hook templates",
        ))
        .stdout(predicate::str::contains(
            "skills    List discovered OMK skills in the local data directory",
        ));
}

#[test]
fn test_version_subcommand() {
    let mut cmd = Command::cargo_bin("omk").unwrap();
    cmd.arg("version");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains(format!(
            "omk {}",
            env!("CARGO_PKG_VERSION")
        )));
}

#[test]
fn test_doctor_runs() {
    let (_tmp, envs) = isolated_env();
    let mut cmd = Command::cargo_bin("omk").unwrap();
    for (k, v) in &envs {
        cmd.env(k, v);
    }
    cmd.arg("doctor");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("omk doctor"))
        .stdout(predicate::str::contains("tmux").not());
}

#[test]
fn test_config_show_runs() {
    let (_tmp, envs) = isolated_env();
    let mut cmd = Command::cargo_bin("omk").unwrap();
    for (k, v) in &envs {
        cmd.env(k, v);
    }
    cmd.arg("config").arg("show");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("omk Configuration"));
}

#[test]
fn test_config_validate_runs() {
    let (_tmp, envs) = isolated_env();
    let mut cmd = Command::cargo_bin("omk").unwrap();
    for (k, v) in &envs {
        cmd.env(k, v);
    }
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
    let (_tmp, envs) = isolated_env();
    let mut cmd = Command::cargo_bin("omk").unwrap();
    for (k, v) in &envs {
        cmd.env(k, v);
    }
    cmd.arg("state").arg("list");
    cmd.assert().success();
}

#[test]
fn test_team_list_runs() {
    let (_tmp, envs) = isolated_env();
    let mut cmd = Command::cargo_bin("omk").unwrap();
    for (k, v) in &envs {
        cmd.env(k, v);
    }
    cmd.arg("team").arg("list");
    cmd.assert().success();
}

#[test]
fn test_skill_list_runs() {
    let (_tmp, envs) = isolated_env();
    let mut cmd = Command::cargo_bin("omk").unwrap();
    for (k, v) in &envs {
        cmd.env(k, v);
    }
    cmd.arg("skill").arg("list");
    cmd.assert().success();
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
    let (_tmp, envs) = isolated_env();
    let mut cmd = Command::cargo_bin("omk").unwrap();
    for (k, v) in &envs {
        cmd.env(k, v);
    }
    cmd.arg("backup").arg("list");
    cmd.assert().success();
}

#[test]
fn test_cleanup_dry_run_runs() {
    let (_tmp, envs) = isolated_env();
    let mut cmd = Command::cargo_bin("omk").unwrap();
    for (k, v) in &envs {
        cmd.env(k, v);
    }
    cmd.arg("cleanup").arg("--dry-run");
    cmd.assert().success();
}

#[test]
fn test_logs_runs() {
    let (_tmp, envs) = isolated_env();
    let mut cmd = Command::cargo_bin("omk").unwrap();
    for (k, v) in &envs {
        cmd.env(k, v);
    }
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
#[ignore = "network-dependent: requires GitHub API access"]
fn test_update_check_runs() {
    let mut cmd = Command::cargo_bin("omk").unwrap();
    cmd.arg("update").arg("--check");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("version").or(predicate::str::contains("Latest")));
}

#[test]
fn test_config_set_team_size() {
    let (_tmp, envs) = isolated_env();
    let mut cmd = Command::cargo_bin("omk").unwrap();
    for (k, v) in &envs {
        cmd.env(k, v);
    }
    cmd.arg("config")
        .arg("set")
        .arg("default_team_size")
        .arg("3");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Set default_team_size = 3"));
}

#[test]
fn test_config_set_invalid_key() {
    let (_tmp, envs) = isolated_env();
    let mut cmd = Command::cargo_bin("omk").unwrap();
    for (k, v) in &envs {
        cmd.env(k, v);
    }
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
fn test_team_spawn_command_is_removed() {
    let mut cmd = Command::cargo_bin("omk").unwrap();
    cmd.arg("team")
        .arg("spawn")
        .arg("1:coder")
        .arg("smoke task");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("unrecognized subcommand 'spawn'"));
}

#[test]
fn test_team_list_ignores_stale_or_empty_state_dirs() {
    let (tmp, envs) = isolated_env();
    let team_root = tmp.path().join("xdg_state").join("omk").join("team");
    fs::create_dir_all(team_root.join("stale-empty")).unwrap();
    let broken = team_root.join("stale-broken");
    fs::create_dir_all(&broken).unwrap();
    fs::write(broken.join("team-state.json"), "not-json").unwrap();

    let mut cmd = Command::cargo_bin("omk").unwrap();
    for (k, v) in &envs {
        cmd.env(k, v);
    }
    cmd.arg("team").arg("list");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("No teams found."));
}

#[test]
fn test_magic_keywords() {
    let mut cmd = Command::cargo_bin("omk").unwrap();
    cmd.arg("t").arg("--help");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("team"));

    let mut cmd = Command::cargo_bin("omk").unwrap();
    cmd.arg("ap").arg("--help");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("autopilot"));

    let mut cmd = Command::cargo_bin("omk").unwrap();
    cmd.arg("r").arg("--help");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("ralph"));
}

#[test]
fn test_team_export_import_roundtrip() {
    let _bin = Command::cargo_bin("omk").unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let export_path = tmp.path().join("test-team.json");

    // Export should fail if team doesn't exist
    let mut cmd = Command::cargo_bin("omk").unwrap();
    cmd.arg("team")
        .arg("export")
        .arg("nonexistent-team")
        .arg("-o")
        .arg(&export_path);
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
fn test_goal_open_pr_help_lists_policy_and_base_branch() {
    let mut cmd = Command::cargo_bin("omk").unwrap();
    cmd.arg("goal").arg("open-pr").arg("--help");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("--policy"))
        .stdout(predicate::str::contains("--base-branch"));
}

#[test]
fn test_goal_merge_help() {
    let mut cmd = Command::cargo_bin("omk").unwrap();
    cmd.arg("goal").arg("merge").arg("--help");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Merge the GitHub PR"));
}

#[path = "fixtures/goal_end_to_end_cli_smoke_basic.rs"]
mod goal_end_to_end_cli_smoke_basic;
#[path = "fixtures/goal_end_to_end_cli_smoke_recovery.rs"]
mod goal_end_to_end_cli_smoke_recovery;
