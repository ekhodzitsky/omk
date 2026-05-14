use assert_cmd::Command;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

fn isolated_env() -> (TempDir, Vec<(&'static str, PathBuf)>) {
    omk::test_helpers::isolated_xdg_env()
}

fn omk_cmd(envs: &[(&'static str, PathBuf)]) -> Command {
    let mut cmd = Command::cargo_bin("omk").expect("omk binary");
    for (key, value) in envs {
        cmd.env(key, value);
    }
    cmd
}

fn xdg_state(envs: &[(&'static str, PathBuf)]) -> PathBuf {
    envs.iter()
        .find_map(|(key, value)| (*key == "XDG_STATE_HOME").then(|| value.clone()))
        .expect("missing XDG_STATE_HOME")
}

fn latest_goal_dir(envs: &[(&'static str, PathBuf)]) -> PathBuf {
    let goals_dir = xdg_state(envs).join("omk").join("goals");
    let mut dirs: Vec<_> = fs::read_dir(goals_dir)
        .expect("missing goals dir")
        .map(|entry| entry.expect("failed to read goal entry").path())
        .filter(|path| path.is_dir())
        .collect();
    dirs.sort();
    dirs.pop().expect("goal dir should exist")
}

#[test]
fn greenfield_plan_writes_acceptance_smoke_demo_oracle_criteria() {
    let (_tmp, envs) = isolated_env();

    omk_cmd(&envs)
        .args([
            "goal",
            "plan",
            "Build a greenfield CLI app with acceptance smoke demo coverage",
        ])
        .assert()
        .success();

    let test_spec =
        fs::read_to_string(latest_goal_dir(&envs).join("test-spec.md")).expect("test spec");
    assert!(test_spec.contains("Oracle kind: `greenfield`"));
    assert!(test_spec.contains("`acceptance`"));
    assert!(test_spec.contains("`smoke`"));
    assert!(test_spec.contains("`demo`"));
    assert!(test_spec.contains("## Readiness Levels"));
    assert!(test_spec.contains("Engineering-ready"));
    assert!(test_spec.contains("Product-ready"));
    let goal_dir = latest_goal_dir(&envs);
    assert!(goal_dir
        .join("artifacts/oracles/greenfield-acceptance.md")
        .exists());
    assert!(goal_dir
        .join("artifacts/oracles/greenfield-demo.sh")
        .exists());
    assert!(goal_dir
        .join("artifacts/oracles/usage-examples.md")
        .exists());
}

#[test]
fn rewrite_plan_writes_compatibility_and_golden_oracle_criteria() {
    let (_tmp, envs) = isolated_env();

    omk_cmd(&envs)
        .args([
            "goal",
            "plan",
            "Rewrite this Python service in Rust with compatibility golden tests",
        ])
        .assert()
        .success();

    let test_spec =
        fs::read_to_string(latest_goal_dir(&envs).join("test-spec.md")).expect("test spec");
    assert!(test_spec.contains("Oracle kind: `rewrite`"));
    assert!(test_spec.contains("`compatibility`"));
    assert!(test_spec.contains("`golden`"));
}

#[test]
fn rewrite_plan_includes_detected_source_surface_compatibility_plan() {
    let (_tmp, envs) = isolated_env();
    let project = TempDir::new().expect("project");
    fs::write(
        project.path().join("Cargo.toml"),
        "[package]\nname='rewrite-demo'\nversion='0.1.0'\nedition='2021'\n",
    )
    .expect("Cargo.toml");
    fs::create_dir_all(project.path().join("src")).expect("src");
    fs::write(
        project.path().join("src/lib.rs"),
        "pub fn answer() -> u8 { 42 }\n",
    )
    .expect("lib.rs");

    omk_cmd(&envs)
        .current_dir(project.path())
        .args([
            "goal",
            "plan",
            "Rewrite this Rust library with compatibility golden tests",
        ])
        .assert()
        .success();

    let test_spec =
        fs::read_to_string(latest_goal_dir(&envs).join("test-spec.md")).expect("test spec");
    assert!(test_spec.contains("## Compatibility Test Plan"));
    assert!(test_spec.contains("cargo test"));
    assert!(test_spec.contains("cargo check --all-targets"));
    assert!(test_spec.contains("`src/lib.rs`"));
}
