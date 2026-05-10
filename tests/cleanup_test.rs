use assert_cmd::Command;

fn isolated_env() -> (tempfile::TempDir, Vec<(&'static str, std::path::PathBuf)>) {
    omk::test_helpers::isolated_xdg_env()
}

fn omk_cmd(envs: &[(&'static str, std::path::PathBuf)]) -> Command {
    let mut cmd = Command::cargo_bin("omk").unwrap();
    for (k, v) in envs {
        cmd.env(k, v);
    }
    cmd
}

#[test]
fn test_cleanup_cli_help() {
    let (_tmp, envs) = isolated_env();
    let output = omk_cmd(&envs)
        .args(["cleanup", "--help"])
        .output()
        .expect("omk failed");

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
    let output = omk_cmd(&envs)
        .args(["cleanup", "--dry-run"])
        .output()
        .expect("omk failed");

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
    let output = omk_cmd(&envs)
        .args(["team", "cleanup", "--help"])
        .output()
        .expect("omk failed");

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
    let output = omk_cmd(&envs)
        .args(["team", "cleanup", "--dry-run"])
        .output()
        .expect("omk failed");

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
    let output = omk_cmd(&envs)
        .args(["cleanup", "--teams", "--dry-run"])
        .output()
        .expect("omk failed");

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
