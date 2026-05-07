use std::process::Command;

#[test]
fn test_config_validate_cli_help() {
    let output = Command::new("cargo")
        .args(["run", "--", "config", "--help"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("cargo run failed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);

    assert!(
        combined.contains("Manage configuration"),
        "config help missing description: {}",
        combined
    );
}

#[test]
fn test_config_show_runs() {
    let output = Command::new("cargo")
        .args(["run", "--", "config", "show"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("cargo run failed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);

    assert!(
        combined.contains("omk Configuration"),
        "config show did not run: {}",
        combined
    );
}

#[test]
fn test_config_validate_runs() {
    let output = Command::new("cargo")
        .args(["run", "--", "config", "validate"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("cargo run failed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);

    assert!(
        combined.contains("Validating omk configuration"),
        "config validate did not run: {}",
        combined
    );
}
