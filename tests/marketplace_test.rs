use std::process::Command;

#[test]
fn test_marketplace_cli_help() {
    let output = Command::new("cargo")
        .args(["run", "--", "marketplace", "--help"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("cargo run failed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);

    assert!(
        combined.contains("Browse skill marketplace"),
        "marketplace help missing description: {}",
        combined
    );
}

#[test]
fn test_marketplace_list_runs() {
    let output = Command::new("cargo")
        .args(["run", "--", "marketplace", "list"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("cargo run failed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);

    assert!(
        output.status.success(),
        "marketplace list failed: {}",
        combined
    );
    assert!(
        combined.contains("omk Marketplace"),
        "marketplace list missing header: {}",
        combined
    );
}
