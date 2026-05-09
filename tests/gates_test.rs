#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

#[tokio::test]
async fn test_detect_gates_rust() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    tokio::fs::write(dir.join("Cargo.toml"), "[package]\n")
        .await
        .unwrap();

    let config = omk::runtime::gates::detect_gates(dir);
    let names: Vec<_> = config.gates.iter().map(|g| g.name.as_str()).collect();
    assert!(
        names.contains(&"format"),
        "Rust preset should include format gate"
    );
    assert!(
        names.contains(&"lint"),
        "Rust preset should include lint gate"
    );
    assert!(
        names.contains(&"tests"),
        "Rust preset should include tests gate"
    );
}

#[tokio::test]
async fn test_detect_gates_node() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    tokio::fs::write(dir.join("package.json"), "{}\n")
        .await
        .unwrap();

    let config = omk::runtime::gates::detect_gates(dir);
    let names: Vec<_> = config.gates.iter().map(|g| g.name.as_str()).collect();
    assert!(
        names.contains(&"tests"),
        "Node preset should include tests gate"
    );
    assert!(
        names.contains(&"lint"),
        "Node preset should include lint gate"
    );
}

#[tokio::test]
async fn test_detect_gates_unknown() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();

    let config = omk::runtime::gates::detect_gates(dir);
    assert!(
        config.gates.is_empty(),
        "Unknown project should have no gates"
    );
}

#[tokio::test]
async fn test_gates_passed_all_required() {
    let results = vec![
        omk::runtime::gates::GateResult {
            name: "fmt".to_string(),
            passed: true,
            stdout: String::new(),
            stderr: String::new(),
            duration_ms: 100,
            required: true,
        },
        omk::runtime::gates::GateResult {
            name: "clippy".to_string(),
            passed: true,
            stdout: String::new(),
            stderr: String::new(),
            duration_ms: 200,
            required: true,
        },
    ];
    assert!(omk::runtime::gates::gates_passed(&results));
}

#[tokio::test]
async fn test_gates_passed_optional_failure_ok() {
    let results = vec![
        omk::runtime::gates::GateResult {
            name: "fmt".to_string(),
            passed: true,
            stdout: String::new(),
            stderr: String::new(),
            duration_ms: 100,
            required: true,
        },
        omk::runtime::gates::GateResult {
            name: "coverage".to_string(),
            passed: false,
            stdout: String::new(),
            stderr: String::new(),
            duration_ms: 200,
            required: false,
        },
    ];
    assert!(omk::runtime::gates::gates_passed(&results));
}

#[tokio::test]
async fn test_gates_passed_required_failure_fails() {
    let results = vec![
        omk::runtime::gates::GateResult {
            name: "fmt".to_string(),
            passed: true,
            stdout: String::new(),
            stderr: String::new(),
            duration_ms: 100,
            required: true,
        },
        omk::runtime::gates::GateResult {
            name: "clippy".to_string(),
            passed: false,
            stdout: String::new(),
            stderr: String::new(),
            duration_ms: 200,
            required: true,
        },
    ];
    assert!(!omk::runtime::gates::gates_passed(&results));
}

#[tokio::test]
async fn test_done_contract_save_and_load() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("contract.json");

    let contract = omk::runtime::gates::DoneContract {
        run_name: "test-run".to_string(),
        mode: "autopilot".to_string(),
        started_at: chrono::Utc::now(),
        completed_at: chrono::Utc::now(),
        gates: vec![],
        changed_files: vec!["src/main.rs".to_string()],
        known_gaps: vec!["docs".to_string()],
        passed: true,
    };

    contract.save(&path).await.unwrap();
    let loaded = omk::runtime::gates::DoneContract::load(&path)
        .await
        .unwrap();

    assert_eq!(loaded.run_name, "test-run");
    assert_eq!(loaded.mode, "autopilot");
    assert!(loaded.passed);
    assert_eq!(loaded.changed_files, vec!["src/main.rs"]);
}

#[tokio::test]
async fn test_run_gates_success() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();

    // Create a script that always succeeds
    let script = dir.join("success.sh");
    #[cfg(unix)]
    {
        tokio::fs::write(&script, "#!/bin/sh\necho ok\n")
            .await
            .unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    #[cfg(windows)]
    {
        tokio::fs::write(&script, "@echo ok\n").await.unwrap();
    }

    let config = omk::runtime::gates::VerificationConfig {
        gates: vec![omk::runtime::gates::GateDef {
            name: "success".to_string(),
            command: script.to_str().unwrap().to_string(),
            args: vec![],
            required: true,
            timeout_secs: 5,
        }],
    };

    let results = omk::runtime::gates::run_gates(&config, dir).await;
    assert_eq!(results.len(), 1);
    assert!(results[0].passed, "Success script should pass");
}

#[tokio::test]
async fn test_run_gates_failure() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();

    // Create a script that always fails
    let script = dir.join("fail.sh");
    #[cfg(unix)]
    {
        tokio::fs::write(&script, "#!/bin/sh\necho error >&2\nexit 1\n")
            .await
            .unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    #[cfg(windows)]
    {
        tokio::fs::write(&script, "@echo error\nexit /b 1\n")
            .await
            .unwrap();
    }

    let config = omk::runtime::gates::VerificationConfig {
        gates: vec![omk::runtime::gates::GateDef {
            name: "fail".to_string(),
            command: script.to_str().unwrap().to_string(),
            args: vec![],
            required: true,
            timeout_secs: 5,
        }],
    };

    let results = omk::runtime::gates::run_gates(&config, dir).await;
    assert_eq!(results.len(), 1);
    assert!(!results[0].passed, "Fail script should not pass");
}
