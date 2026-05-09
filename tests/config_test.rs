use std::process::Command;

fn isolated_env() -> (tempfile::TempDir, Vec<(&'static str, std::path::PathBuf)>) {
    omk::test_helpers::isolated_xdg_env()
}

fn env_path(envs: &[(&'static str, std::path::PathBuf)], key: &str) -> std::path::PathBuf {
    envs.iter()
        .find_map(|(k, v)| (*k == key).then(|| v.clone()))
        .expect("missing isolated env path")
}

#[test]
fn test_config_validate_cli_help() {
    let (_tmp, envs) = isolated_env();
    let mut cmd = Command::new("cargo");
    for (k, v) in &envs {
        cmd.env(k, v);
    }
    let output = cmd
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
    let (_tmp, envs) = isolated_env();
    let mut cmd = Command::new("cargo");
    for (k, v) in &envs {
        cmd.env(k, v);
    }
    let output = cmd
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
    let (_tmp, envs) = isolated_env();
    let mut cmd = Command::new("cargo");
    for (k, v) in &envs {
        cmd.env(k, v);
    }
    let output = cmd
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

#[test]
fn test_config_set_creates_config_dir_before_write() {
    let (_tmp, envs) = isolated_env();
    let config_home = env_path(&envs, "XDG_CONFIG_HOME");
    let config_dir = config_home.join("omk");
    let config_file = config_dir.join("config.toml");

    assert!(
        !config_dir.exists(),
        "config dir should not exist before test: {}",
        config_dir.display()
    );

    let mut cmd = Command::new("cargo");
    for (k, v) in &envs {
        cmd.env(k, v);
    }
    let output = cmd
        .args(["run", "--", "config", "set", "default_yolo", "true"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("cargo run failed");

    assert!(
        output.status.success(),
        "config set failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        config_dir.exists(),
        "config dir was not created: {}",
        config_dir.display()
    );
    assert!(
        config_file.exists(),
        "config file was not created: {}",
        config_file.display()
    );

    let content = std::fs::read_to_string(&config_file).expect("failed to read config file");
    assert!(
        content.contains("default_yolo = true"),
        "unexpected config content: {}",
        content
    );
}
