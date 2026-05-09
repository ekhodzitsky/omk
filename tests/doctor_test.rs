use std::process::Command;

fn isolated_env() -> (tempfile::TempDir, Vec<(&'static str, std::path::PathBuf)>) {
    omk::test_helpers::isolated_xdg_env()
}

#[test]
fn test_doctor_cli_help() {
    let (_tmp, envs) = isolated_env();
    let mut cmd = Command::new("cargo");
    for (k, v) in &envs {
        cmd.env(k, v);
    }
    let output = cmd
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
    let (_tmp, envs) = isolated_env();
    let mut cmd = Command::new("cargo");
    for (k, v) in &envs {
        cmd.env(k, v);
    }
    let output = cmd
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
