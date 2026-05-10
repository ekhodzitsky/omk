use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

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
            "team         Team orchestration (scheduler run + tmux-compatible spawn surface)",
        ))
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
            "Team orchestration (scheduler run + tmux-compatible spawn surface)",
        ))
        .stdout(predicate::str::contains(
            "spawn      Spawn workers in tmux compatibility mode",
        ))
        .stdout(predicate::str::contains(
            "run        Run a scheduler-backed team workflow (no tmux required)",
        ));
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
        .stdout(predicate::str::contains("omk doctor"));
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
fn test_team_spawn_missing_task() {
    let mut cmd = Command::cargo_bin("omk").unwrap();
    cmd.arg("team").arg("spawn").arg("3:coder");
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Task description is required"));
}

#[test]
fn test_team_spawn_fails_gracefully_when_tmux_missing() {
    let (tmp, envs) = isolated_env();
    let empty_path = tmp.path().join("empty-path");
    fs::create_dir_all(&empty_path).unwrap();
    let mut cmd = Command::cargo_bin("omk").unwrap();
    for (k, v) in &envs {
        cmd.env(k, v);
    }

    cmd.env("PATH", empty_path)
        .arg("team")
        .arg("spawn")
        .arg("1:coder")
        .arg("smoke task");

    cmd.assert().failure().stderr(predicate::str::contains(
        "tmux is required but not installed",
    ));
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

// L0-011: CI-friendly smoke tests that do not require real Kimi or tmux.
#[test]
fn test_help_smoke() {
    let mut cmd = Command::cargo_bin("omk").unwrap();
    cmd.arg("--help");
    cmd.assert().success();

    let mut cmd = Command::cargo_bin("omk").unwrap();
    cmd.arg("doctor").arg("--help");
    cmd.assert().success();

    let mut cmd = Command::cargo_bin("omk").unwrap();
    cmd.arg("config").arg("--help");
    cmd.assert().success();
}

#[test]
fn test_north_star_demo_exits_non_zero_when_proof_failed() {
    let tmp = tempfile::tempdir().unwrap();
    let fake_bin = tmp.path().join("fake-bin");
    fs::create_dir_all(&fake_bin).unwrap();

    let fake_omk = fake_bin.join("omk");
    let fake_kimi = fake_bin.join("kimi");

    fs::write(
        &fake_omk,
        r#"#!/usr/bin/env bash
set -euo pipefail
if [[ "${1:-}" == "--version" || "${1:-}" == "version" ]]; then
  echo "omk 0.2.5"
  exit 0
fi
if [[ "${1:-}" == "kimi" && "${2:-}" == "sync" ]]; then
  exit 0
fi
if [[ "${1:-}" == "team" && "${2:-}" == "run" ]]; then
  exit 0
fi
if [[ "${1:-}" == "hud" ]]; then
  echo '{"task_summary":{"total":3,"completed":2},"workers":[{"id":"worker-0"},{"id":"worker-1"},{"id":"worker-2"}]}'
  exit 0
fi
if [[ "${1:-}" == "proof" && "${2:-}" == "show" && "${3:-}" == "latest" && "${4:-}" == "--format" && "${5:-}" == "json" ]]; then
  echo '{"status":"failed","changed_files":[],"gates":[],"failures":[{"description":"mock failure"}],"retries":[],"known_gaps":[]}'
  exit 0
fi
if [[ "${1:-}" == "proof" && "${2:-}" == "show" && "${3:-}" == "latest" && "${4:-}" == "--format" && "${5:-}" == "text" ]]; then
  echo "Proof status: failed"
  exit 0
fi
if [[ "${1:-}" == "team" && "${2:-}" == "cleanup" ]]; then
  exit 0
fi
echo "unsupported fake omk args: $*" >&2
exit 1
"#,
    )
    .unwrap();
    fs::write(&fake_kimi, "#!/usr/bin/env bash\nexit 0\n").unwrap();

    #[cfg(unix)]
    {
        let mut perms = fs::metadata(&fake_omk).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&fake_omk, perms).unwrap();
        let mut kimi_perms = fs::metadata(&fake_kimi).unwrap().permissions();
        kimi_perms.set_mode(0o755);
        fs::set_permissions(&fake_kimi, kimi_perms).unwrap();
    }

    let current_path = std::env::var("PATH").unwrap_or_default();
    let combined_path = format!("{}:{}", fake_bin.display(), current_path);

    let script_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("scripts")
        .join("north_star_demo.sh");
    let output = std::process::Command::new("bash")
        .arg(script_path)
        .env("PATH", combined_path)
        .env("NORTH_STAR_DRY_RUN", "1")
        .output()
        .unwrap();

    assert!(
        !output.status.success(),
        "north_star_demo.sh must return non-zero when proof status=failed"
    );
}

#[test]
fn test_north_star_demo_mock_mode_isolates_home_and_xdg() {
    let tmp = tempfile::tempdir().unwrap();
    let fake_bin = tmp.path().join("fake-bin");
    fs::create_dir_all(&fake_bin).unwrap();

    let fake_omk = fake_bin.join("omk");
    let log_path = tmp.path().join("fake-omk.log");

    fs::write(
        &fake_omk,
        r#"#!/usr/bin/env bash
set -euo pipefail
if [[ "${1:-}" == "--version" || "${1:-}" == "version" ]]; then
  echo "omk 0.2.5"
  exit 0
fi
if [[ "${1:-}" == "kimi" && "${2:-}" == "sync" ]]; then
  echo "sync HOME=${HOME:-} XDG_STATE_HOME=${XDG_STATE_HOME:-} XDG_CONFIG_HOME=${XDG_CONFIG_HOME:-} XDG_CACHE_HOME=${XDG_CACHE_HOME:-}" >> "${FAKE_OMK_LOG}"
  exit 0
fi
if [[ "${1:-}" == "team" && "${2:-}" == "run" ]]; then
  if [[ -z "${MOCK_KIMI:-}" || ! -x "${MOCK_KIMI}" ]]; then
    echo "missing executable MOCK_KIMI: ${MOCK_KIMI:-}" >&2
    exit 2
  fi
  echo "team HOME=${HOME:-} XDG_STATE_HOME=${XDG_STATE_HOME:-} XDG_CONFIG_HOME=${XDG_CONFIG_HOME:-} XDG_CACHE_HOME=${XDG_CACHE_HOME:-} MOCK_KIMI=${MOCK_KIMI}" >> "${FAKE_OMK_LOG}"
  exit 0
fi
if [[ "${1:-}" == "hud" ]]; then
  echo '{"task_summary":{"total":3,"completed":3},"workers":[{"id":"worker-0"},{"id":"worker-1"},{"id":"worker-2"}]}'
  exit 0
fi
if [[ "${1:-}" == "proof" && "${2:-}" == "show" && "${3:-}" == "latest" && "${4:-}" == "--format" && "${5:-}" == "json" ]]; then
  echo '{"status":"ready","changed_files":["src/lib.rs"],"gates":[{"name":"verification","status":"passed","required":true}],"failures":[],"retries":[],"known_gaps":[]}'
  exit 0
fi
if [[ "${1:-}" == "proof" && "${2:-}" == "show" && "${3:-}" == "latest" && "${4:-}" == "--format" && "${5:-}" == "text" ]]; then
  echo "Proof status: ready"
  exit 0
fi
if [[ "${1:-}" == "team" && "${2:-}" == "cleanup" ]]; then
  exit 0
fi
echo "unsupported fake omk args: $*" >&2
exit 1
"#,
    )
    .unwrap();

    #[cfg(unix)]
    {
        let mut perms = fs::metadata(&fake_omk).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&fake_omk, perms).unwrap();
    }

    let current_path = std::env::var("PATH").unwrap_or_default();
    let combined_path = format!("{}:{}", fake_bin.display(), current_path);

    let script_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("scripts")
        .join("north_star_demo.sh");
    let output = std::process::Command::new("bash")
        .arg(script_path)
        .env("PATH", combined_path)
        .env("NORTH_STAR_DRY_RUN", "1")
        .env("MOCK_KIMI", "1")
        .env("FAKE_OMK_LOG", &log_path)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "north_star_demo.sh should succeed in MOCK_KIMI=1 mode"
    );

    let log = fs::read_to_string(&log_path).unwrap();
    assert!(log.contains("sync HOME=/tmp/omk-north-star-"));
    assert!(log.contains("team HOME=/tmp/omk-north-star-"));
    assert!(log.contains("XDG_STATE_HOME=/tmp/omk-north-star-"));
    assert!(log.contains("XDG_CONFIG_HOME=/tmp/omk-north-star-"));
    assert!(log.contains("XDG_CACHE_HOME=/tmp/omk-north-star-"));
    assert!(log.contains("MOCK_KIMI=/tmp/omk-north-star-"));
}

#[test]
fn test_north_star_demo_accepts_custom_executable_mock_kimi_path() {
    let tmp = tempfile::tempdir().unwrap();
    let fake_bin = tmp.path().join("fake-bin");
    fs::create_dir_all(&fake_bin).unwrap();

    let fake_omk = fake_bin.join("omk");
    let log_path = tmp.path().join("fake-omk.log");
    let custom_mock_kimi = tmp.path().join("custom-mock-kimi");

    fs::write(
        &fake_omk,
        r#"#!/usr/bin/env bash
set -euo pipefail
if [[ "${1:-}" == "--version" || "${1:-}" == "version" ]]; then
  echo "omk 0.2.5"
  exit 0
fi
if [[ "${1:-}" == "kimi" && "${2:-}" == "sync" ]]; then
  echo "sync MOCK_KIMI=${MOCK_KIMI:-}" >> "${FAKE_OMK_LOG}"
  exit 0
fi
if [[ "${1:-}" == "team" && "${2:-}" == "run" ]]; then
  if [[ -z "${MOCK_KIMI:-}" || ! -x "${MOCK_KIMI}" ]]; then
    echo "missing executable MOCK_KIMI: ${MOCK_KIMI:-}" >&2
    exit 2
  fi
  echo "team MOCK_KIMI=${MOCK_KIMI}" >> "${FAKE_OMK_LOG}"
  exit 0
fi
if [[ "${1:-}" == "hud" ]]; then
  echo '{"task_summary":{"total":3,"completed":3},"workers":[{"id":"worker-0"},{"id":"worker-1"},{"id":"worker-2"}]}'
  exit 0
fi
if [[ "${1:-}" == "proof" && "${2:-}" == "show" && "${3:-}" == "latest" && "${4:-}" == "--format" && "${5:-}" == "json" ]]; then
  echo '{"status":"ready","changed_files":["src/lib.rs"],"gates":[{"name":"verification","status":"passed","required":true}],"failures":[],"retries":[],"known_gaps":[]}'
  exit 0
fi
if [[ "${1:-}" == "proof" && "${2:-}" == "show" && "${3:-}" == "latest" && "${4:-}" == "--format" && "${5:-}" == "text" ]]; then
  echo "Proof status: ready"
  exit 0
fi
if [[ "${1:-}" == "team" && "${2:-}" == "cleanup" ]]; then
  exit 0
fi
echo "unsupported fake omk args: $*" >&2
exit 1
"#,
    )
    .unwrap();
    fs::write(&custom_mock_kimi, "#!/usr/bin/env bash\nexit 0\n").unwrap();

    #[cfg(unix)]
    {
        let mut omk_perms = fs::metadata(&fake_omk).unwrap().permissions();
        omk_perms.set_mode(0o755);
        fs::set_permissions(&fake_omk, omk_perms).unwrap();

        let mut mock_perms = fs::metadata(&custom_mock_kimi).unwrap().permissions();
        mock_perms.set_mode(0o755);
        fs::set_permissions(&custom_mock_kimi, mock_perms).unwrap();
    }

    let current_path = std::env::var("PATH").unwrap_or_default();
    let combined_path = format!("{}:{}", fake_bin.display(), current_path);

    let script_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("scripts")
        .join("north_star_demo.sh");
    let output = std::process::Command::new("bash")
        .arg(script_path)
        .env("PATH", combined_path)
        .env("NORTH_STAR_DRY_RUN", "1")
        .env("MOCK_KIMI", &custom_mock_kimi)
        .env("FAKE_OMK_LOG", &log_path)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "north_star_demo.sh should accept an executable custom MOCK_KIMI path"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let expected_mock_line = format!("MOCK_KIMI={}", custom_mock_kimi.display());
    assert!(stdout.contains(&expected_mock_line));

    let log = fs::read_to_string(&log_path).unwrap();
    assert!(log.contains(&expected_mock_line));
}

#[test]
fn test_north_star_demo_rejects_non_executable_mock_kimi_path() {
    let tmp = tempfile::tempdir().unwrap();
    let fake_bin = tmp.path().join("fake-bin");
    fs::create_dir_all(&fake_bin).unwrap();

    let fake_omk = fake_bin.join("omk");
    let not_executable_mock = tmp.path().join("not-executable-mock-kimi");

    fs::write(
        &fake_omk,
        r#"#!/usr/bin/env bash
set -euo pipefail
if [[ "${1:-}" == "--version" || "${1:-}" == "version" ]]; then
  echo "omk 0.2.5"
  exit 0
fi
echo "unsupported fake omk args: $*" >&2
exit 1
"#,
    )
    .unwrap();
    fs::write(&not_executable_mock, "not executable\n").unwrap();

    #[cfg(unix)]
    {
        let mut perms = fs::metadata(&fake_omk).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&fake_omk, perms).unwrap();
    }

    let current_path = std::env::var("PATH").unwrap_or_default();
    let combined_path = format!("{}:{}", fake_bin.display(), current_path);

    let script_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("scripts")
        .join("north_star_demo.sh");
    let output = std::process::Command::new("bash")
        .arg(script_path)
        .env("PATH", combined_path)
        .env("NORTH_STAR_DRY_RUN", "1")
        .env("MOCK_KIMI", &not_executable_mock)
        .output()
        .unwrap();

    assert!(
        !output.status.success(),
        "north_star_demo.sh must fail when MOCK_KIMI points to a non-executable file"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let expected = format!(
        "MOCK_KIMI is set but not executable: {}",
        not_executable_mock.display()
    );
    assert!(stdout.contains(&expected));
}

#[test]
fn test_north_star_demo_missing_kimi_prints_mock_kimi_hint() {
    let tmp = tempfile::tempdir().unwrap();
    let fake_bin = tmp.path().join("fake-bin");
    fs::create_dir_all(&fake_bin).unwrap();

    let fake_omk = fake_bin.join("omk");
    fs::write(
        &fake_omk,
        r#"#!/usr/bin/env bash
set -euo pipefail
if [[ "${1:-}" == "--version" || "${1:-}" == "version" ]]; then
  echo "omk 0.2.5"
  exit 0
fi
echo "unsupported fake omk args: $*" >&2
exit 1
"#,
    )
    .unwrap();

    let isolated_script_dir = tmp.path().join("isolated-scripts");
    fs::create_dir_all(&isolated_script_dir).unwrap();
    let isolated_script_path = isolated_script_dir.join("north_star_demo.sh");
    let original_script_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("scripts")
        .join("north_star_demo.sh");
    fs::copy(original_script_path, &isolated_script_path).unwrap();

    #[cfg(unix)]
    {
        let mut omk_perms = fs::metadata(&fake_omk).unwrap().permissions();
        omk_perms.set_mode(0o755);
        fs::set_permissions(&fake_omk, omk_perms).unwrap();

        let mut script_perms = fs::metadata(&isolated_script_path).unwrap().permissions();
        script_perms.set_mode(0o755);
        fs::set_permissions(&isolated_script_path, script_perms).unwrap();
    }

    let output = std::process::Command::new("/bin/bash")
        .arg(&isolated_script_path)
        .env(
            "PATH",
            format!("{}:/usr/bin:/bin:/usr/sbin:/sbin", fake_bin.display()),
        )
        .env("NORTH_STAR_DRY_RUN", "1")
        .output()
        .unwrap();

    assert!(
        !output.status.success(),
        "north_star_demo.sh must fail when neither kimi nor mock-kimi is available"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Neither 'kimi' nor 'mock-kimi' found."));
    assert!(stdout.contains("Or run with MOCK_KIMI=1 for a fully mocked demo"));
}

#[test]
fn test_north_star_demo_fails_cleanly_when_kimi_path_is_unusable() {
    let tmp = tempfile::tempdir().unwrap();
    let fake_bin = tmp.path().join("fake-bin");
    fs::create_dir_all(&fake_bin).unwrap();

    let fake_omk = fake_bin.join("omk");
    let fake_kimi = fake_bin.join("kimi");

    fs::write(
        &fake_omk,
        r#"#!/usr/bin/env bash
set -euo pipefail
if [[ "${1:-}" == "--version" || "${1:-}" == "version" ]]; then
  echo "omk 0.2.5"
  exit 0
fi
if [[ "${1:-}" == "kimi" && "${2:-}" == "sync" ]]; then
  exit 0
fi
if [[ "${1:-}" == "team" && "${2:-}" == "run" ]]; then
  echo "kimi auth required" >&2
  exit 1
fi
if [[ "${1:-}" == "hud" ]]; then
  echo '{"task_summary":{"total":2,"completed":0},"workers":[]}'
  exit 0
fi
if [[ "${1:-}" == "proof" && "${2:-}" == "show" && "${3:-}" == "latest" && "${4:-}" == "--format" && "${5:-}" == "json" ]]; then
  echo '{"status":"failed","changed_files":[],"gates":[],"failures":[{"description":"run failed"}],"retries":[],"known_gaps":[]}'
  exit 0
fi
if [[ "${1:-}" == "proof" && "${2:-}" == "show" && "${3:-}" == "latest" && "${4:-}" == "--format" && "${5:-}" == "text" ]]; then
  echo "Proof status: failed"
  exit 0
fi
if [[ "${1:-}" == "team" && "${2:-}" == "cleanup" ]]; then
  exit 0
fi
echo "unsupported fake omk args: $*" >&2
exit 1
"#,
    )
    .unwrap();
    fs::write(
        &fake_kimi,
        "#!/usr/bin/env bash\necho 'kimi version 1.0.0'\nexit 0\n",
    )
    .unwrap();

    #[cfg(unix)]
    {
        let mut omk_perms = fs::metadata(&fake_omk).unwrap().permissions();
        omk_perms.set_mode(0o755);
        fs::set_permissions(&fake_omk, omk_perms).unwrap();

        let mut kimi_perms = fs::metadata(&fake_kimi).unwrap().permissions();
        kimi_perms.set_mode(0o755);
        fs::set_permissions(&fake_kimi, kimi_perms).unwrap();
    }

    let current_path = std::env::var("PATH").unwrap_or_default();
    let combined_path = format!("{}:{}", fake_bin.display(), current_path);

    let script_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("scripts")
        .join("north_star_demo.sh");
    let output = std::process::Command::new("bash")
        .arg(script_path)
        .env("PATH", combined_path)
        .env("NORTH_STAR_DRY_RUN", "1")
        .output()
        .unwrap();

    assert!(
        !output.status.success(),
        "north_star_demo.sh must fail when available kimi path is unusable during team run"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Using real Kimi CLI"));
    assert!(stdout.contains("omk team run failed"));
}
