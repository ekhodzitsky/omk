use assert_cmd::Command;

fn isolated_env() -> (tempfile::TempDir, Vec<(&'static str, std::path::PathBuf)>) {
    omk::test_helpers::isolated_xdg_env()
}

fn omk_cmd(envs: &[(&'static str, std::path::PathBuf)]) -> Command {
    let mut cmd = Command::cargo_bin("omk").unwrap();
    for (k, v) in envs {
        cmd.env(k, v);
    }
    cmd
}

#[test]
fn test_backup_cli_help() {
    let (_tmp, envs) = isolated_env();
    let output = omk_cmd(&envs)
        .args(["backup", "--help"])
        .output()
        .expect("omk failed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);

    assert!(
        combined.contains("Backup and restore state"),
        "backup help missing description: {}",
        combined
    );
}

#[test]
fn test_backup_create_list() {
    // Use the same isolated environment for both create and list.
    let (_tmp, envs) = isolated_env();

    // Create a backup
    let output = omk_cmd(&envs)
        .args(["backup", "create", "--name", "test-backup"])
        .output()
        .expect("omk failed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);

    assert!(
        output.status.success(),
        "backup create failed: {}",
        combined
    );

    // List backups
    let output = omk_cmd(&envs)
        .args(["backup", "list"])
        .output()
        .expect("omk failed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);

    assert!(output.status.success(), "backup list failed: {}", combined);
    assert!(
        combined.contains("omk-backup-test-backup.tar.gz"),
        "backup list missing test backup: {}",
        combined
    );
}
