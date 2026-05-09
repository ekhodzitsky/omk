use assert_cmd::Command;
use predicates::str::contains;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_kimi_native_help() {
    let mut cmd = Command::cargo_bin("omk").unwrap();
    cmd.arg("kimi").arg("--help");
    cmd.assert()
        .success()
        .stdout(contains("sync"))
        .stdout(contains("doctor"))
        .stdout(contains("install"))
        .stdout(contains("rollback"))
        .stdout(contains("agents"))
        .stdout(contains("hooks"))
        .stdout(contains("skills"));
}

#[test]
fn test_kimi_native_alias_k() {
    let mut cmd = Command::cargo_bin("omk").unwrap();
    cmd.arg("k").arg("--help");
    cmd.assert().success().stdout(contains("sync"));
}

#[test]
fn test_kimi_sync_creates_agents_and_hooks() {
    let tmp = TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("omk").unwrap();
    cmd.current_dir(&tmp);
    cmd.arg("kimi").arg("sync");
    cmd.assert().success().stdout(contains("Sync complete"));

    // Check agents were created
    let agents_dir = tmp.path().join(".kimi").join("agents");
    assert!(agents_dir.join("architect").join("agent.yaml").exists());
    assert!(agents_dir.join("architect").join("system.md").exists());
    assert!(agents_dir.join("executor").join("agent.yaml").exists());

    // Check hooks were created
    let hooks_dir = tmp.path().join(".kimi").join("hooks");
    assert!(hooks_dir.join("safety-check.sh").exists());
    assert!(hooks_dir.join("completion-check.sh").exists());
    assert!(hooks_dir.join("notify.sh").exists());
}

#[test]
fn test_kimi_sync_creates_manifest() {
    let tmp = TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("omk").unwrap();
    cmd.current_dir(&tmp);
    cmd.arg("kimi").arg("sync");
    cmd.assert().success();

    let manifest = tmp.path().join(".kimi").join("omk-manifest.json");
    assert!(manifest.exists());
    let content = fs::read_to_string(&manifest).unwrap();
    assert!(content.contains("agent_spec"));
    assert!(content.contains("agent_prompt"));
    assert!(content.contains("hook_script"));
}

#[test]
fn test_kimi_doctor_reports_missing() {
    let tmp = TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("omk").unwrap();
    cmd.current_dir(&tmp);
    cmd.arg("kimi").arg("doctor");
    cmd.assert()
        .success()
        .stdout(contains("Kimi-native doctor"))
        .stdout(contains("issue(s) found"));
}

#[test]
fn test_kimi_doctor_passes_after_sync() {
    let tmp = TempDir::new().unwrap();

    // First sync
    let mut sync_cmd = Command::cargo_bin("omk").unwrap();
    sync_cmd.current_dir(&tmp);
    sync_cmd.arg("kimi").arg("sync");
    sync_cmd.assert().success();

    // Create AGENTS.md so doctor passes fully
    fs::write(tmp.path().join("AGENTS.md"), "# Test\n").unwrap();

    // Then doctor
    let mut cmd = Command::cargo_bin("omk").unwrap();
    cmd.current_dir(&tmp);
    cmd.arg("kimi").arg("doctor");
    cmd.assert().success().stdout(contains("All checks passed"));
}

#[test]
fn test_kimi_doctor_detects_drift() {
    let tmp = TempDir::new().unwrap();

    // Sync
    let mut sync_cmd = Command::cargo_bin("omk").unwrap();
    sync_cmd.current_dir(&tmp);
    sync_cmd.arg("kimi").arg("sync");
    sync_cmd.assert().success();

    // Delete a file to simulate drift
    fs::remove_file(tmp.path().join(".kimi/agents/architect/agent.yaml")).unwrap();

    // Doctor should detect drift
    let mut cmd = Command::cargo_bin("omk").unwrap();
    cmd.current_dir(&tmp);
    cmd.arg("kimi").arg("doctor");
    cmd.assert()
        .success()
        .stdout(contains("Missing manifest file"));
}

#[test]
fn test_kimi_agents_lists_roles() {
    let mut cmd = Command::cargo_bin("omk").unwrap();
    cmd.arg("kimi").arg("agents");
    cmd.assert()
        .success()
        .stdout(contains("architect"))
        .stdout(contains("executor"))
        .stdout(contains("verifier"));
}

#[test]
fn test_kimi_hooks_lists_events() {
    let mut cmd = Command::cargo_bin("omk").unwrap();
    cmd.arg("kimi").arg("hooks");
    cmd.assert()
        .success()
        .stdout(contains("PreToolUse"))
        .stdout(contains("safety-check.sh"));
}

#[test]
fn test_kimi_install_creates_assets() {
    let tmp = TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("omk").unwrap();
    cmd.current_dir(&tmp);
    cmd.arg("kimi").arg("install");
    cmd.assert()
        .success()
        .stdout(contains("Installation complete"));

    assert!(tmp.path().join(".kimi").join("agents").exists());
    assert!(tmp.path().join(".kimi").join("hooks").exists());
    assert!(tmp.path().join(".kimi").join("hooks.toml.example").exists());
}

#[test]
fn test_kimi_rollback_removes_assets() {
    let tmp = TempDir::new().unwrap();

    // Install
    let mut install_cmd = Command::cargo_bin("omk").unwrap();
    install_cmd.current_dir(&tmp);
    install_cmd.arg("kimi").arg("install");
    install_cmd.assert().success();

    assert!(tmp
        .path()
        .join(".kimi/agents/architect/agent.yaml")
        .exists());

    // Rollback
    let mut rollback_cmd = Command::cargo_bin("omk").unwrap();
    rollback_cmd.current_dir(&tmp);
    rollback_cmd.arg("kimi").arg("rollback");
    rollback_cmd
        .assert()
        .success()
        .stdout(contains("Rollback complete"));

    assert!(!tmp
        .path()
        .join(".kimi/agents/architect/agent.yaml")
        .exists());
    assert!(!tmp.path().join(".kimi/omk-manifest.json").exists());
}

#[test]
fn test_kimi_sync_creates_backup_on_overwrite() {
    let tmp = TempDir::new().unwrap();

    // First sync
    let mut sync_cmd = Command::cargo_bin("omk").unwrap();
    sync_cmd.current_dir(&tmp);
    sync_cmd.arg("kimi").arg("sync");
    sync_cmd.assert().success();

    // Modify a hook file
    let hook_path = tmp.path().join(".kimi/hooks/safety-check.sh");
    let original = fs::read_to_string(&hook_path).unwrap();
    fs::write(&hook_path, "# modified\n").unwrap();

    // Sync again
    let mut sync_cmd = Command::cargo_bin("omk").unwrap();
    sync_cmd.current_dir(&tmp);
    sync_cmd.arg("kimi").arg("sync");
    sync_cmd.assert().success();

    // Check backup was created
    let hooks_dir = tmp.path().join(".kimi/hooks");
    let backups: Vec<_> = fs::read_dir(&hooks_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().contains(".omk-backup-"))
        .collect();
    assert!(!backups.is_empty(), "Expected backup file to be created");

    // Verify backup contains the modified content
    let backup_path = backups[0].path();
    let backup_content = fs::read_to_string(&backup_path).unwrap();
    assert_eq!(backup_content, "# modified\n");

    // Verify the file was restored to original content
    let restored_content = fs::read_to_string(&hook_path).unwrap();
    assert_eq!(restored_content, original);
}

#[test]
fn test_kimi_rollback_no_manifest() {
    let tmp = TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("omk").unwrap();
    cmd.current_dir(&tmp);
    cmd.arg("kimi").arg("rollback");
    cmd.assert()
        .success()
        .stdout(contains("Nothing to rollback"));
}

#[test]
fn test_kimi_sync_dry_run_no_files_written() {
    let tmp = TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("omk").unwrap();
    cmd.current_dir(&tmp);
    cmd.arg("kimi").arg("sync").arg("--dry-run");
    cmd.assert().success().stdout(contains("Dry run"));

    // No .kimi directory should be created
    assert!(!tmp.path().join(".kimi").exists());
}

#[test]
fn test_kimi_rollback_dry_run_no_files_deleted() {
    let tmp = TempDir::new().unwrap();

    // Install
    let mut install_cmd = Command::cargo_bin("omk").unwrap();
    install_cmd.current_dir(&tmp);
    install_cmd.arg("kimi").arg("install");
    install_cmd.assert().success();

    assert!(tmp
        .path()
        .join(".kimi/agents/architect/agent.yaml")
        .exists());

    // Rollback dry-run
    let mut rollback_cmd = Command::cargo_bin("omk").unwrap();
    rollback_cmd.current_dir(&tmp);
    rollback_cmd.arg("kimi").arg("rollback").arg("--dry-run");
    rollback_cmd
        .assert()
        .success()
        .stdout(contains("Dry run"))
        .stdout(contains("Skipped:"))
        .stdout(contains("Report: restored="))
        .stdout(contains("errors=0"));

    // Files should still exist
    assert!(tmp
        .path()
        .join(".kimi/agents/architect/agent.yaml")
        .exists());
    assert!(tmp.path().join(".kimi/omk-manifest.json").exists());
}

#[test]
fn test_kimi_rollback_restores_backup() {
    let tmp = TempDir::new().unwrap();

    // First sync
    let mut sync_cmd = Command::cargo_bin("omk").unwrap();
    sync_cmd.current_dir(&tmp);
    sync_cmd.arg("kimi").arg("sync");
    sync_cmd.assert().success();

    // Modify a hook file
    let hook_path = tmp.path().join(".kimi/hooks/safety-check.sh");
    fs::write(&hook_path, "# modified by user\n").unwrap();

    // Sync again to create a backup
    let mut sync_cmd = Command::cargo_bin("omk").unwrap();
    sync_cmd.current_dir(&tmp);
    sync_cmd.arg("kimi").arg("sync");
    sync_cmd.assert().success();

    // Verify backup was created
    let hooks_dir = tmp.path().join(".kimi/hooks");
    let backups: Vec<_> = fs::read_dir(&hooks_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().contains(".omk-backup-"))
        .collect();
    assert!(!backups.is_empty(), "Expected backup file to be created");

    // Rollback should restore the backup
    let mut rollback_cmd = Command::cargo_bin("omk").unwrap();
    rollback_cmd.current_dir(&tmp);
    rollback_cmd.arg("kimi").arg("rollback");
    rollback_cmd
        .assert()
        .success()
        .stdout(contains("Restored from backup"));

    // The hook should now contain the user's modified content
    let content = fs::read_to_string(&hook_path).unwrap();
    assert_eq!(content, "# modified by user\n");
}

#[test]
fn test_kimi_doctor_json_output() {
    let tmp = TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("omk").unwrap();
    cmd.current_dir(&tmp);
    cmd.arg("kimi").arg("doctor").arg("--json");
    cmd.assert()
        .success()
        .stdout(contains("\"severity\""))
        .stdout(contains("\"message\""));
}

#[test]
fn test_kimi_sync_skips_identical_files() {
    let tmp = TempDir::new().unwrap();

    // First sync
    let mut sync_cmd = Command::cargo_bin("omk").unwrap();
    sync_cmd.current_dir(&tmp);
    sync_cmd.arg("kimi").arg("sync");
    sync_cmd.assert().success();

    // Second sync should skip all identical files
    let mut sync_cmd = Command::cargo_bin("omk").unwrap();
    sync_cmd.current_dir(&tmp);
    sync_cmd.arg("kimi").arg("sync");
    sync_cmd.assert().success().stdout(contains("unchanged"));

    // No new backups should be created
    let hooks_dir = tmp.path().join(".kimi/hooks");
    let backups: Vec<_> = fs::read_dir(&hooks_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().contains(".omk-backup-"))
        .collect();
    assert!(
        backups.is_empty(),
        "Expected no backups for identical files"
    );
}

#[test]
fn test_kimi_doctor_detects_invalid_agent() {
    let tmp = TempDir::new().unwrap();

    // Sync first
    let mut sync_cmd = Command::cargo_bin("omk").unwrap();
    sync_cmd.current_dir(&tmp);
    sync_cmd.arg("kimi").arg("sync");
    sync_cmd.assert().success();

    // Corrupt an agent spec
    let agent_yaml = tmp.path().join(".kimi/agents/architect/agent.yaml");
    fs::write(&agent_yaml, "not: valid yaml {{").unwrap();

    // Doctor should detect invalid YAML
    let mut cmd = Command::cargo_bin("omk").unwrap();
    cmd.current_dir(&tmp);
    cmd.arg("kimi").arg("doctor");
    cmd.assert().success().stdout(contains("invalid YAML"));
}

#[test]
fn test_kimi_doctor_detects_dangling_hook_config() {
    let tmp = TempDir::new().unwrap();

    // Sync first
    let mut sync_cmd = Command::cargo_bin("omk").unwrap();
    sync_cmd.current_dir(&tmp);
    sync_cmd.arg("kimi").arg("sync");
    sync_cmd.assert().success();

    // Create a hooks.toml.example with a dangling reference
    let hooks_toml = tmp.path().join(".kimi/hooks.toml.example");
    fs::write(
        &hooks_toml,
        r#"[[hooks]]
event = "PreToolUse"
command = ".kimi/hooks/missing.sh"
"#,
    )
    .unwrap();

    // Doctor should detect missing script reference
    let mut cmd = Command::cargo_bin("omk").unwrap();
    cmd.current_dir(&tmp);
    cmd.arg("kimi").arg("doctor");
    cmd.assert().success().stdout(contains("missing scripts"));
}
