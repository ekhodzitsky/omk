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
fn test_marketplace_cli_help() {
    let (_tmp, envs) = isolated_env();
    let output = omk_cmd(&envs)
        .args(["marketplace", "--help"])
        .output()
        .expect("omk failed");

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
    let (_tmp, envs) = isolated_env();
    let output = omk_cmd(&envs)
        .args(["marketplace", "list"])
        .output()
        .expect("omk failed");

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
