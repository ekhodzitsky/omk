use std::process::Command;

fn isolated_env() -> (tempfile::TempDir, Vec<(&'static str, std::path::PathBuf)>) {
    omk::test_helpers::isolated_xdg_env()
}

#[test]
fn test_state_cli_help() {
    let (_tmp, envs) = isolated_env();
    let mut cmd = Command::new("cargo");
    for (k, v) in &envs {
        cmd.env(k, v);
    }
    let output = cmd
        .args(["run", "--", "state", "--help"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("cargo run failed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);

    assert!(
        combined.contains("Export/import state"),
        "state help missing description: {}",
        combined
    );
}

#[test]
fn test_state_export_runs() {
    let (_tmp, envs) = isolated_env();
    let mut cmd = Command::new("cargo");
    for (k, v) in &envs {
        cmd.env(k, v);
    }
    let output = cmd
        .args([
            "run",
            "--",
            "state",
            "export",
            "--output",
            "/tmp/omk-test-export.json",
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("cargo run failed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);

    assert!(output.status.success(), "state export failed: {}", combined);
    assert!(
        combined.contains("State exported"),
        "state export did not complete: {}",
        combined
    );

    // Cleanup
    let _ = std::fs::remove_file("/tmp/omk-test-export.json");
}
