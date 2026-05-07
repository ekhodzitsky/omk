use std::process::Command;

#[test]
fn test_state_cli_help() {
    let output = Command::new("cargo")
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
    let output = Command::new("cargo")
        .args(["run", "--", "state", "export", "--output", "/tmp/omk-test-export.json"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("cargo run failed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);

    assert!(
        output.status.success(),
        "state export failed: {}",
        combined
    );
    assert!(
        combined.contains("State exported"),
        "state export did not complete: {}",
        combined
    );

    // Cleanup
    let _ = std::fs::remove_file("/tmp/omk-test-export.json");
}
