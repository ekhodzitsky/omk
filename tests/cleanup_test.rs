use std::process::Command;

#[test]
fn test_cleanup_cli_help() {
    let output = Command::new("cargo")
        .args(["run", "--", "cleanup", "--help"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("cargo run failed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);

    assert!(
        combined.contains("Clean up old state files"),
        "cleanup help missing description: {}",
        combined
    );
}

#[test]
fn test_cleanup_dry_run() {
    let output = Command::new("cargo")
        .args(["run", "--", "cleanup", "--dry-run"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("cargo run failed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);

    // Dry run should complete without error even if nothing to clean
    assert!(
        output.status.success(),
        "cleanup --dry-run failed: {}",
        combined
    );
}
