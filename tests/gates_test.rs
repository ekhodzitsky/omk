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
        names.contains(&"check"),
        "Rust preset should include check gate"
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
            command_line: "cargo fmt --check".to_string(),
            exit_code: Some(0),
            timed_out: false,
            stdout_summary: None,
            stderr_summary: None,
            output_path: None,
            timeout_secs: 120,
        },
        omk::runtime::gates::GateResult {
            name: "clippy".to_string(),
            passed: true,
            stdout: String::new(),
            stderr: String::new(),
            duration_ms: 200,
            required: true,
            command_line: "cargo clippy -- -D warnings".to_string(),
            exit_code: Some(0),
            timed_out: false,
            stdout_summary: None,
            stderr_summary: None,
            output_path: None,
            timeout_secs: 120,
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
            command_line: "cargo fmt --check".to_string(),
            exit_code: Some(0),
            timed_out: false,
            stdout_summary: None,
            stderr_summary: None,
            output_path: None,
            timeout_secs: 120,
        },
        omk::runtime::gates::GateResult {
            name: "coverage".to_string(),
            passed: false,
            stdout: String::new(),
            stderr: String::new(),
            duration_ms: 200,
            required: false,
            command_line: "cargo tarpaulin".to_string(),
            exit_code: Some(1),
            timed_out: false,
            stdout_summary: None,
            stderr_summary: None,
            output_path: None,
            timeout_secs: 120,
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
            command_line: "cargo fmt --check".to_string(),
            exit_code: Some(0),
            timed_out: false,
            stdout_summary: None,
            stderr_summary: None,
            output_path: None,
            timeout_secs: 120,
        },
        omk::runtime::gates::GateResult {
            name: "clippy".to_string(),
            passed: false,
            stdout: String::new(),
            stderr: String::new(),
            duration_ms: 200,
            required: true,
            command_line: "cargo clippy -- -D warnings".to_string(),
            exit_code: Some(1),
            timed_out: false,
            stdout_summary: None,
            stderr_summary: None,
            output_path: None,
            timeout_secs: 120,
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

#[tokio::test]
async fn test_run_gates_with_evidence_writes_output_artifact_and_metadata() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    let artifacts = dir.join("artifacts");
    tokio::fs::create_dir_all(&artifacts).await.unwrap();

    let script = dir.join("evidence.sh");
    #[cfg(unix)]
    {
        tokio::fs::write(
            &script,
            "#!/bin/sh\necho line-1\necho line-2\necho err-1 >&2\nexit 7\n",
        )
        .await
        .unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    #[cfg(windows)]
    {
        tokio::fs::write(
            &script,
            "@echo line-1\r\n@echo line-2\r\n@echo err-1 1>&2\r\nexit /b 7\r\n",
        )
        .await
        .unwrap();
    }

    let config = omk::runtime::gates::VerificationConfig {
        gates: vec![omk::runtime::gates::GateDef {
            name: "evidence".to_string(),
            command: script.to_str().unwrap().to_string(),
            args: vec![],
            required: true,
            timeout_secs: 5,
        }],
    };

    let results =
        omk::runtime::gates::run_gates_with_evidence(&config, dir, Some(&artifacts)).await;
    assert_eq!(results.len(), 1);
    let gate = &results[0];
    assert!(!gate.passed);
    assert_eq!(gate.exit_code, Some(7));
    assert!(!gate.timed_out);
    assert!(gate.command_line.contains("evidence"));
    assert!(gate
        .stdout_summary
        .as_deref()
        .unwrap_or_default()
        .contains("line-1"));
    assert!(gate
        .stderr_summary
        .as_deref()
        .unwrap_or_default()
        .contains("err-1"));
    let output_path = gate.output_path.as_ref().expect("expected output path");
    assert!(
        std::path::Path::new(output_path).exists(),
        "full output artifact should exist"
    );
    let full_output = std::fs::read_to_string(output_path).unwrap();
    assert!(full_output.contains("line-1"));
    assert!(full_output.contains("err-1"));
}

#[tokio::test]
async fn test_run_gates_with_evidence_marks_timeout_without_artifact_dir() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    let script = dir.join("timeout.sh");
    #[cfg(unix)]
    {
        tokio::fs::write(&script, "#!/bin/sh\nsleep 2\necho done\n")
            .await
            .unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    #[cfg(windows)]
    {
        tokio::fs::write(&script, "@ping 127.0.0.1 -n 3 > nul\r\n@echo done\r\n")
            .await
            .unwrap();
    }

    let config = omk::runtime::gates::VerificationConfig {
        gates: vec![omk::runtime::gates::GateDef {
            name: "timeout".to_string(),
            command: script.to_str().unwrap().to_string(),
            args: vec![],
            required: true,
            timeout_secs: 1,
        }],
    };

    let results = omk::runtime::gates::run_gates_with_evidence(&config, dir, None).await;
    assert_eq!(results.len(), 1);
    let gate = &results[0];
    assert!(!gate.passed);
    assert!(gate.timed_out);
    assert_eq!(gate.exit_code, None);
    assert!(gate.output_path.is_none());
    assert!(gate
        .stderr_summary
        .as_deref()
        .unwrap_or_default()
        .contains("Timed out after 1s"));
}

#[tokio::test]
async fn test_run_gates_keeps_compatibility_without_evidence_artifact() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();

    let script = dir.join("compat.sh");
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
            name: "compat".to_string(),
            command: script.to_str().unwrap().to_string(),
            args: vec![],
            required: true,
            timeout_secs: 5,
        }],
    };

    let results = omk::runtime::gates::run_gates(&config, dir).await;
    assert_eq!(results.len(), 1);
    assert!(results[0].passed);
    assert!(results[0].output_path.is_none());
}

#[tokio::test]
async fn test_load_or_detect_gates_supports_custom_gate_without_args() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    let omk_dir = dir.join(".omk");
    tokio::fs::create_dir_all(&omk_dir).await.unwrap();
    tokio::fs::write(
        omk_dir.join("gates.toml"),
        r#"
[[gates]]
name = "custom"
command = "echo"
required = true
timeout_secs = 9
"#,
    )
    .await
    .unwrap();

    let config = omk::runtime::gates::load_or_detect_gates(dir).await;
    assert_eq!(config.gates.len(), 1);
    let gate = &config.gates[0];
    assert_eq!(gate.name, "custom");
    assert_eq!(gate.command, "echo");
    assert!(gate.args.is_empty());
    assert!(gate.required);
    assert_eq!(gate.timeout_secs, 9);
}

#[tokio::test]
async fn test_load_or_detect_gates_supports_allow_fail_and_skip_semantics() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    let omk_dir = dir.join(".omk");
    tokio::fs::create_dir_all(&omk_dir).await.unwrap();

    let fail_script = dir.join("fail.sh");
    #[cfg(unix)]
    {
        tokio::fs::write(&fail_script, "#!/bin/sh\nexit 1\n")
            .await
            .unwrap();
        std::fs::set_permissions(&fail_script, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    #[cfg(windows)]
    {
        tokio::fs::write(&fail_script, "@exit /b 1\n")
            .await
            .unwrap();
    }

    tokio::fs::write(
        omk_dir.join("gates.toml"),
        format!(
            r#"
[[gates]]
name = "allow-fail"
command = "{}"
allow-fail = true
timeout_secs = 5

[[gates]]
name = "skipped"
command = "definitely-not-a-real-command-omk"
skip = true
required = true
timeout_secs = 5
"#,
            fail_script.display()
        ),
    )
    .await
    .unwrap();

    let config = omk::runtime::gates::load_or_detect_gates(dir).await;
    assert_eq!(config.gates.len(), 2);

    let results = omk::runtime::gates::run_gates(&config, dir).await;
    assert_eq!(results.len(), 2);

    let allow_fail = results.iter().find(|g| g.name == "allow-fail").unwrap();
    assert!(!allow_fail.passed);
    assert!(!allow_fail.required);

    let skipped = results.iter().find(|g| g.name == "skipped").unwrap();
    assert!(skipped.passed);
    assert!(!skipped.required);
    assert!(skipped.command_line.contains("skipped"));
    assert!(omk::runtime::gates::gates_passed(&results));
}
