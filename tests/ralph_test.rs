use std::path::PathBuf;
use std::process::Command;

fn omk_bin() -> PathBuf {
    std::env::var("CARGO_BIN_EXE_omk")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("target")
                .join("debug")
                .join("omk")
        })
}

#[test]
fn test_ralph_cli_help() {
    let output = Command::new(omk_bin())
        .args(["ralph", "--help"])
        .output()
        .expect("Failed to run omk ralph --help");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Persistent") || stdout.contains("verify/fix"));
}

#[test]
fn test_ralph_cli_requires_task() {
    let output = Command::new(omk_bin())
        .args(["ralph"])
        .output()
        .expect("Failed to run omk ralph");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("required") || stderr.contains("Task"),
        "Expected error about missing task. Got: {}",
        stderr
    );
}
