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
fn test_ask_cli_help() {
    let output = Command::new(omk_bin())
        .args(["ask", "--help"])
        .output()
        .expect("Failed to run omk ask --help");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("provider"));
}

#[test]
fn test_ask_cli_requires_prompt() {
    let output = Command::new(omk_bin())
        .args(["ask", "kimi"])
        .output()
        .expect("Failed to run omk ask");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("required") || stderr.contains("Prompt"),
        "Expected error about missing prompt. Got: {}",
        stderr
    );
}
