use std::path::PathBuf;
use tempfile::TempDir;

fn omk_bin() -> PathBuf {
    std::env::var("CARGO_BIN_EXE_omk")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("target")
                .join("debug")
                .join("omk")
        })
}

async fn setup_temp_dir() -> TempDir {
    TempDir::new().unwrap()
}

#[tokio::test]
async fn test_autopilot_creates_state_and_plan() {
    let dir = setup_temp_dir().await;
    let bin = omk_bin();

    let output = std::process::Command::new(&bin)
        .args([
            "autopilot",
            "fix bug",
            "-d",
            dir.path().to_str().unwrap(),
            "--name",
            "test-basic",
        ])
        .output()
        .expect("Failed to run omk autopilot");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "omk autopilot failed. stdout: {}\nstderr: {}",
        stdout,
        stderr
    );

    let state_file = dir
        .path()
        .join(".omk")
        .join("state")
        .join("autopilot")
        .join("test-basic")
        .join("autopilot-state.json");
    assert!(
        state_file.exists(),
        "State file should exist: {:?}",
        state_file
    );

    let json = tokio::fs::read_to_string(&state_file).await.unwrap();
    let state: serde_json::Value = serde_json::from_str(&json).unwrap();

    assert_eq!(state["phase"], "Complete");
    assert_eq!(state["task"], "fix bug");
    assert!(state["created_at"].as_str().is_some());

    let plan_file = dir
        .path()
        .join(".omk")
        .join("plans")
        .join("autopilot-test-basic-plan.md");
    assert!(
        plan_file.exists(),
        "Plan file should exist: {:?}",
        plan_file
    );
}

#[tokio::test]
async fn test_autopilot_complex_task() {
    let dir = setup_temp_dir().await;
    let bin = omk_bin();

    let long_task = "implement feature X and refactor the database layer and update the API endpoints and write tests";
    let output = std::process::Command::new(&bin)
        .args([
            "autopilot",
            long_task,
            "-d",
            dir.path().to_str().unwrap(),
            "--name",
            "test-complex",
        ])
        .output()
        .expect("Failed to run omk autopilot");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "omk autopilot failed. stdout: {}\nstderr: {}",
        stdout,
        stderr
    );

    let state_file = dir
        .path()
        .join(".omk")
        .join("state")
        .join("autopilot")
        .join("test-complex")
        .join("autopilot-state.json");
    let json = tokio::fs::read_to_string(&state_file).await.unwrap();
    let state: serde_json::Value = serde_json::from_str(&json).unwrap();

    assert_eq!(state["phase"], "Complete");
    assert!(state["task"].as_str().unwrap().contains("and"));
}

#[tokio::test]
async fn test_autopilot_qa_results_for_rust_project() {
    let dir = setup_temp_dir().await;
    let bin = omk_bin();

    // Create a minimal Cargo.toml to trigger Rust QA path
    let cargo_toml = dir.path().join("Cargo.toml");
    tokio::fs::write(
        &cargo_toml,
        "[package]\nname = \"test\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .await
    .unwrap();

    let output = std::process::Command::new(&bin)
        .args([
            "autopilot",
            "refactor code",
            "-d",
            dir.path().to_str().unwrap(),
            "--name",
            "test-qa",
        ])
        .output()
        .expect("Failed to run omk autopilot");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "omk autopilot failed. stdout: {}\nstderr: {}",
        stdout,
        stderr
    );

    let state_file = dir
        .path()
        .join(".omk")
        .join("state")
        .join("autopilot")
        .join("test-qa")
        .join("autopilot-state.json");
    let json = tokio::fs::read_to_string(&state_file).await.unwrap();
    let state: serde_json::Value = serde_json::from_str(&json).unwrap();

    assert_eq!(state["phase"], "Complete");
    assert!(
        state["qa_results"].is_object(),
        "QA results should be populated"
    );
    assert!(
        state["validation_results"].is_array(),
        "Validation results should be an array"
    );
    assert_eq!(
        state["validation_results"].as_array().unwrap().len(),
        2,
        "Expected architect + security reviews"
    );
}
