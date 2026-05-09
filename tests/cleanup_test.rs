use std::process::Command;

fn isolated_env() -> (tempfile::TempDir, Vec<(&'static str, std::path::PathBuf)>) {
    omk::test_helpers::isolated_xdg_env()
}

#[test]
fn test_cleanup_cli_help() {
    let (_tmp, envs) = isolated_env();
    let mut cmd = Command::new("cargo");
    for (k, v) in &envs {
        cmd.env(k, v);
    }
    let output = cmd
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
    let (_tmp, envs) = isolated_env();
    let mut cmd = Command::new("cargo");
    for (k, v) in &envs {
        cmd.env(k, v);
    }
    let output = cmd
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

#[test]
fn test_team_cleanup_cli_help() {
    let (_tmp, envs) = isolated_env();
    let mut cmd = Command::new("cargo");
    for (k, v) in &envs {
        cmd.env(k, v);
    }
    let output = cmd
        .args(["run", "--", "team", "cleanup", "--help"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("cargo run failed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);

    assert!(
        combined.contains("Clean up old team state directories"),
        "team cleanup help missing description: {}",
        combined
    );
}

#[test]
fn test_team_cleanup_dry_run() {
    let (_tmp, envs) = isolated_env();
    let mut cmd = Command::new("cargo");
    for (k, v) in &envs {
        cmd.env(k, v);
    }
    let output = cmd
        .args(["run", "--", "team", "cleanup", "--dry-run"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("cargo run failed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);

    assert!(
        output.status.success(),
        "team cleanup --dry-run failed: {}",
        combined
    );
}

#[test]
fn test_cleanup_teams_flag() {
    let (_tmp, envs) = isolated_env();
    let mut cmd = Command::new("cargo");
    for (k, v) in &envs {
        cmd.env(k, v);
    }
    let output = cmd
        .args(["run", "--", "cleanup", "--teams", "--dry-run"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("cargo run failed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);

    assert!(
        output.status.success(),
        "cleanup --teams --dry-run failed: {}",
        combined
    );
    assert!(
        combined.contains("team state directories"),
        "Expected team-specific summary: {}",
        combined
    );
}
