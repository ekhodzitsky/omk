use std::process::Command;

#[test]
fn test_doctor_cli_help() {
    let output = Command::new("cargo")
        .args(["run", "--", "doctor", "--help"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("cargo run failed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);

    assert!(
        combined.contains("Diagnose environment and dependencies"),
        "doctor help missing description: {}",
        combined
    );
}

#[test]
fn test_doctor_runs() {
    let output = Command::new("cargo")
        .args(["run", "--", "doctor"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("cargo run failed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);

    assert!(
        combined.contains("omk doctor"),
        "doctor command did not run: {}",
        combined
    );
}
