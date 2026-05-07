use std::process::Command;

#[test]
fn test_skill_cli_help() {
    let output = Command::new("cargo")
        .args(["run", "--", "skill", "--help"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("cargo run failed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);

    assert!(
        combined.contains("Manage skills"),
        "skill help missing description: {}",
        combined
    );
}

#[test]
fn test_skill_list_runs() {
    let output = Command::new("cargo")
        .args(["run", "--", "skill", "list"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("cargo run failed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);

    assert!(
        output.status.success(),
        "skill list failed: {}",
        combined
    );
}
