use std::process::Command;

#[test]
fn test_backup_cli_help() {
    let output = Command::new("cargo")
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
    // Create a backup
    let output = Command::new("cargo")
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
    let output = Command::new("cargo")
        .args(["run", "--", "backup", "list"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("cargo run failed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);

    assert!(
        output.status.success(),
        "backup list failed: {}",
        combined
    );
    assert!(
        combined.contains("omk-backup-test-backup.tar.gz"),
        "backup list missing test backup: {}",
        combined
    );
}
