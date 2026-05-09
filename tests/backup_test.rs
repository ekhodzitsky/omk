use std::process::Command;

fn isolated_env() -> (tempfile::TempDir, Vec<(&'static str, std::path::PathBuf)>) {
    omk::test_helpers::isolated_xdg_env()
}

#[test]
fn test_backup_cli_help() {
    let (_tmp, envs) = isolated_env();
    let mut cmd = Command::new("cargo");
    for (k, v) in &envs {
        cmd.env(k, v);
    }
    let output = cmd
        .args(["run", "--", "backup", "--help"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("cargo run failed");

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
    let mut cmd = Command::new("cargo");
    for (k, v) in &envs {
        cmd.env(k, v);
    }
    let output = cmd
        .args(["run", "--", "backup", "create", "--name", "test-backup"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("cargo run failed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);

    assert!(
        output.status.success(),
        "backup create failed: {}",
        combined
    );

    // List backups
    let mut cmd = Command::new("cargo");
    for (k, v) in &envs {
        cmd.env(k, v);
    }
    let output = cmd
        .args(["run", "--", "backup", "list"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("cargo run failed");

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
