#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

fn single_gate_config(
    name: &str,
    command: &str,
    args: Vec<String>,
    required: bool,
    timeout_secs: u64,
) -> omk::runtime::gates::VerificationConfig {
    omk::runtime::gates::VerificationConfig {
        gates: vec![omk::runtime::gates::GateDef {
            name: name.to_string(),
            command: command.to_string(),
            args,
            required,
            timeout_secs,
            circuit_breaker: None,
        }],
    }
}

#[cfg(unix)]
fn write_unix_script(path: &std::path::Path, body: &str) {
    // Synchronous I/O on purpose: a 100-byte test fixture has no benefit from
    // tokio's blocking-pool indirection, and using std::fs avoids subtle
    // file-handle / close-ordering interactions with the subsequent spawn
    // under heavy parallel-test load.
    std::fs::write(path, body).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

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
            circuit_breaker_open: false,
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
            circuit_breaker_open: false,
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
            circuit_breaker_open: false,
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
            circuit_breaker_open: false,
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
            circuit_breaker_open: false,
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
            circuit_breaker_open: false,
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

    let script = dir.join("success.sh");
    #[cfg(unix)]
    write_unix_script(&script, "#!/bin/sh\necho ok\n");
    #[cfg(windows)]
    tokio::fs::write(&script, "@echo ok\n").await.unwrap();

    let config = single_gate_config("success", script.to_str().unwrap(), vec![], true, 5);

    let results = omk::runtime::gates::run_gates(&config, dir).await;
    assert_eq!(results.len(), 1);
    assert!(results[0].passed, "Success script should pass");
}

#[tokio::test]
async fn test_run_gates_failure() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();

    let script = dir.join("fail.sh");
    #[cfg(unix)]
    write_unix_script(&script, "#!/bin/sh\necho error >&2\nexit 1\n");
    #[cfg(windows)]
    tokio::fs::write(&script, "@echo error\nexit /b 1\n")
        .await
        .unwrap();

    let config = single_gate_config("fail", script.to_str().unwrap(), vec![], true, 5);

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
    write_unix_script(
        &script,
        "#!/bin/sh\necho line-1\necho line-2\necho err-1 >&2\nexit 7\n",
    );
    #[cfg(windows)]
    tokio::fs::write(
        &script,
        "@echo line-1\r\n@echo line-2\r\n@echo err-1 1>&2\r\nexit /b 7\r\n",
    )
    .await
    .unwrap();

    let config = single_gate_config("evidence", script.to_str().unwrap(), vec![], true, 5);

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

#[cfg(unix)]
#[tokio::test]
async fn test_run_gates_with_evidence_drains_large_output_before_waiting() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    let artifacts = dir.join("artifacts");
    tokio::fs::create_dir_all(&artifacts).await.unwrap();

    let script = dir.join("large-output.sh");
    write_unix_script(
        &script,
        r#"#!/bin/sh
awk 'BEGIN {
  for (i = 0; i < 2000; i++) {
    printf "stdout-line-%05d\n", i
    printf "stderr-line-%05d\n", i > "/dev/stderr"
  }
  exit 7
}'
"#,
    );

    let config = single_gate_config("large-output", script.to_str().unwrap(), vec![], true, 30);

    let results =
        omk::runtime::gates::run_gates_with_evidence(&config, dir, Some(&artifacts)).await;
    assert_eq!(results.len(), 1);
    let gate = &results[0];
    assert!(!gate.passed);
    assert_eq!(gate.exit_code, Some(7));
    assert!(!gate.timed_out);
    let output_path = gate.output_path.as_ref().expect("expected output path");
    let full_output = std::fs::read_to_string(output_path).unwrap();
    assert!(full_output.contains("stdout-line-00000"));
    assert!(full_output.contains("stderr-line-00000"));
}

#[tokio::test]
async fn test_run_gates_with_evidence_marks_timeout_without_artifact_dir() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    let script = dir.join("timeout.sh");
    #[cfg(unix)]
    write_unix_script(&script, "#!/bin/sh\nsleep 3600\necho done\n");
    #[cfg(windows)]
    tokio::fs::write(&script, "@ping 127.0.0.1 -n 3601 > nul\r\n@echo done\r\n")
        .await
        .unwrap();

    let config = single_gate_config("timeout", script.to_str().unwrap(), vec![], true, 3);

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
        .contains("Timed out after 3s"));
}

#[tokio::test]
async fn test_run_gates_keeps_compatibility_without_evidence_artifact() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();

    let script = dir.join("compat.sh");
    #[cfg(unix)]
    write_unix_script(&script, "#!/bin/sh\necho ok\n");
    #[cfg(windows)]
    tokio::fs::write(&script, "@echo ok\n").await.unwrap();

    let config = single_gate_config("compat", script.to_str().unwrap(), vec![], true, 5);

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
    write_unix_script(&fail_script, "#!/bin/sh\nexit 1\n");
    #[cfg(windows)]
    tokio::fs::write(&fail_script, "@exit /b 1\n")
        .await
        .unwrap();

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

#[tokio::test]
async fn test_run_gates_empty_config_returns_no_results() {
    let tmp = tempfile::tempdir().unwrap();
    let config = omk::runtime::gates::VerificationConfig { gates: vec![] };
    let results = omk::runtime::gates::run_gates(&config, tmp.path()).await;
    assert!(
        results.is_empty(),
        "empty gate config should yield no results, got {results:?}"
    );
}

#[tokio::test]
async fn test_run_gates_missing_command_with_timeout_reports_run_error() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    let config = single_gate_config(
        "missing",
        "this-binary-definitely-does-not-exist-omk-test",
        vec!["--probe".to_string()],
        true,
        5,
    );

    let results = omk::runtime::gates::run_gates(&config, dir).await;
    assert_eq!(results.len(), 1);
    let g = &results[0];
    assert!(!g.passed);
    assert_eq!(g.exit_code, None);
    assert!(!g.timed_out);
    assert!(g.output_path.is_none());
    assert!(g.required);
    assert_eq!(g.timeout_secs, 5);
    assert_eq!(
        g.command_line, "this-binary-definitely-does-not-exist-omk-test --probe",
        "command_line must equal render_command_line(cmd, args) exactly",
    );
    assert!(
        g.stderr.starts_with("Run error: "),
        "stderr should start with the documented explicit-timeout error prefix, got {:?}",
        g.stderr
    );
    assert_eq!(
        g.stderr_summary.as_deref(),
        Some(g.stderr.as_str()),
        "error-path summary must equal raw stderr (make_gate_error contract)",
    );
    assert!(g.stdout.is_empty(), "stdout should be empty on spawn error");
    assert!(
        g.stdout_summary.is_none(),
        "stdout_summary should be None when stdout is empty",
    );
}

#[tokio::test]
async fn test_run_gates_missing_command_default_timeout_reports_spawn_error() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    let config = single_gate_config(
        "missing-default-timeout",
        "this-binary-definitely-does-not-exist-omk-test",
        vec![],
        false,
        0,
    );

    let results = omk::runtime::gates::run_gates(&config, dir).await;
    assert_eq!(results.len(), 1);
    let g = &results[0];
    assert!(!g.passed);
    assert_eq!(g.exit_code, None);
    assert!(!g.timed_out);
    assert!(g.output_path.is_none());
    assert!(!g.required);
    assert_eq!(
        g.timeout_secs, 0,
        "timeout_secs == 0 must propagate verbatim"
    );
    assert_eq!(
        g.command_line, "this-binary-definitely-does-not-exist-omk-test",
        "render_command_line with empty args must equal the bare command",
    );
    assert!(
        g.stderr.starts_with("Spawn error: "),
        "stderr should start with the documented default-timeout error prefix, got {:?}",
        g.stderr
    );
    assert_eq!(
        g.stderr_summary.as_deref(),
        Some(g.stderr.as_str()),
        "error-path summary must equal raw stderr (make_gate_error contract)",
    );
    assert!(g.stdout.is_empty(), "stdout should be empty on spawn error");
    assert!(g.stdout_summary.is_none());
}

#[tokio::test]
async fn test_run_gates_skipped_gate_has_stable_result_shape() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    let omk_dir = dir.join(".omk");
    tokio::fs::create_dir_all(&omk_dir).await.unwrap();
    tokio::fs::write(
        omk_dir.join("gates.toml"),
        r#"
[[gates]]
name = "skipped-only"
command = "echo"
args = ["should-not-appear", "neither-this"]
skip = true
required = true
timeout_secs = 3
"#,
    )
    .await
    .unwrap();

    let config = omk::runtime::gates::load_or_detect_gates(dir).await;
    let results = omk::runtime::gates::run_gates(&config, dir).await;
    assert_eq!(results.len(), 1);
    let gate = &results[0];
    assert_eq!(gate.name, "skipped-only");
    assert!(gate.passed, "skipped gate must report passed=true");
    assert!(!gate.required, "skip forces required=false");
    assert!(gate.stdout.is_empty());
    assert_eq!(gate.stderr, "Skipped by gate config");
    // The skipped-gate command_line is a fixed placeholder — configured args
    // must NOT leak into the rendered command line, or downstream proof
    // consumers would treat skipped gates as if they had executed.
    assert_eq!(gate.command_line, "<skipped by config>");
    assert!(
        !gate.command_line.contains("should-not-appear"),
        "skipped gate must hide args from command_line, got {:?}",
        gate.command_line,
    );
    assert!(!gate.timed_out);
    assert_eq!(gate.exit_code, None);
    assert!(gate.output_path.is_none());
    assert_eq!(gate.timeout_secs, 3);
    assert!(gate.stdout_summary.is_none());
    assert_eq!(
        gate.stderr_summary.as_deref(),
        Some("Skipped by gate config")
    );
}

#[tokio::test]
async fn test_load_or_detect_gates_falls_back_on_empty_gate_name() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    let omk_dir = dir.join(".omk");
    tokio::fs::create_dir_all(&omk_dir).await.unwrap();
    tokio::fs::write(
        omk_dir.join("gates.toml"),
        r#"
[[gates]]
name = ""
command = "echo"
required = true
timeout_secs = 5
"#,
    )
    .await
    .unwrap();

    let config = omk::runtime::gates::load_or_detect_gates(dir).await;
    assert!(
        config.gates.is_empty(),
        "invalid gates.toml (empty name) must fall back to auto-detect for unknown project"
    );
}

#[tokio::test]
async fn test_load_or_detect_gates_falls_back_on_empty_command() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    let omk_dir = dir.join(".omk");
    tokio::fs::create_dir_all(&omk_dir).await.unwrap();
    tokio::fs::write(
        omk_dir.join("gates.toml"),
        r#"
[[gates]]
name = "bad-cmd"
command = ""
required = true
timeout_secs = 5
"#,
    )
    .await
    .unwrap();

    let config = omk::runtime::gates::load_or_detect_gates(dir).await;
    assert!(
        config.gates.is_empty(),
        "invalid gates.toml (empty command) must fall back to auto-detect for unknown project"
    );
}

#[tokio::test]
async fn test_load_or_detect_gates_falls_back_on_duplicate_names() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    let omk_dir = dir.join(".omk");
    tokio::fs::create_dir_all(&omk_dir).await.unwrap();
    tokio::fs::write(
        omk_dir.join("gates.toml"),
        r#"
[[gates]]
name = "dup"
command = "echo"
required = true
timeout_secs = 5

[[gates]]
name = "dup"
command = "echo"
required = true
timeout_secs = 5
"#,
    )
    .await
    .unwrap();

    let config = omk::runtime::gates::load_or_detect_gates(dir).await;
    assert!(
        config.gates.is_empty(),
        "invalid gates.toml (duplicate names) must fall back to auto-detect for unknown project"
    );
}

#[tokio::test]
async fn test_load_or_detect_gates_falls_back_on_timeout_too_large() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    let omk_dir = dir.join(".omk");
    tokio::fs::create_dir_all(&omk_dir).await.unwrap();
    tokio::fs::write(
        omk_dir.join("gates.toml"),
        r#"
[[gates]]
name = "huge-timeout"
command = "echo"
required = true
timeout_secs = 86401
"#,
    )
    .await
    .unwrap();

    let config = omk::runtime::gates::load_or_detect_gates(dir).await;
    assert!(
        config.gates.is_empty(),
        "invalid gates.toml (timeout > 86400) must fall back to auto-detect for unknown project"
    );
}

#[tokio::test]
async fn test_load_or_detect_gates_accepts_max_timeout() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    let omk_dir = dir.join(".omk");
    tokio::fs::create_dir_all(&omk_dir).await.unwrap();
    tokio::fs::write(
        omk_dir.join("gates.toml"),
        r#"
[[gates]]
name = "max-timeout"
command = "echo"
required = true
timeout_secs = 86400
"#,
    )
    .await
    .unwrap();

    let config = omk::runtime::gates::load_or_detect_gates(dir).await;
    assert_eq!(config.gates.len(), 1);
    assert_eq!(config.gates[0].name, "max-timeout");
    assert_eq!(config.gates[0].timeout_secs, 86400);
}

#[cfg(unix)]
#[tokio::test]
async fn test_run_gates_captures_exit_code_stdout_stderr_and_args() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();

    let script = dir.join("nonzero.sh");
    write_unix_script(
        &script,
        "#!/bin/sh\necho \"out:$1\"\necho \"err:$2\" >&2\nexit 42\n",
    );

    let config = single_gate_config(
        "nonzero",
        script.to_str().unwrap(),
        vec!["alpha".to_string(), "beta".to_string()],
        true,
        5,
    );

    let results = omk::runtime::gates::run_gates(&config, dir).await;
    assert_eq!(results.len(), 1);
    let g = &results[0];
    assert!(!g.passed);
    assert_eq!(g.exit_code, Some(42));
    assert!(!g.timed_out);
    assert!(g.required);
    assert_eq!(g.timeout_secs, 5);
    assert!(
        g.stdout.contains("out:alpha"),
        "stdout should capture script output verbatim, got {:?}",
        g.stdout
    );
    assert!(
        g.stderr.contains("err:beta"),
        "stderr should capture script stderr verbatim, got {:?}",
        g.stderr
    );
    assert!(
        g.stdout_summary
            .as_deref()
            .unwrap_or_default()
            .contains("out:alpha"),
        "stdout_summary should reflect the first lines"
    );
    assert!(
        g.stderr_summary
            .as_deref()
            .unwrap_or_default()
            .contains("err:beta"),
        "stderr_summary should reflect the first lines"
    );
    let expected_command_line = format!("{} alpha beta", script.display());
    assert_eq!(
        g.command_line, expected_command_line,
        "command_line must equal render_command_line(cmd, args) exactly",
    );
    assert!(g.output_path.is_none(), "no artifact dir → no output_path");
}

#[tokio::test]
async fn test_gate_result_json_uses_stable_field_names() {
    // Canonical on-disk shape that proof/state consumers depend on. Any rename
    // (e.g. command_line → commandLine via #[serde(rename)]) must break this
    // test — a symmetric round-trip would not, because it would silently
    // serialize and deserialize through the same alias.
    let canonical = r#"{
        "name": "fmt",
        "passed": false,
        "stdout": "out-line-1\nout-line-2",
        "stderr": "err-line-1",
        "duration_ms": 1234,
        "required": true,
        "command_line": "cargo fmt --check",
        "exit_code": 101,
        "timed_out": false,
        "stdout_summary": "out-line-1",
        "stderr_summary": "err-line-1",
        "output_path": "/tmp/log",
        "timeout_secs": 30,
        "circuit_breaker_open": false
    }"#;
    let parsed: omk::runtime::gates::GateResult =
        serde_json::from_str(canonical).expect("canonical JSON must deserialize");

    assert_eq!(parsed.name, "fmt");
    assert!(!parsed.passed);
    assert_eq!(parsed.stdout, "out-line-1\nout-line-2");
    assert_eq!(parsed.stderr, "err-line-1");
    assert_eq!(parsed.duration_ms, 1234);
    assert!(parsed.required);
    assert_eq!(parsed.command_line, "cargo fmt --check");
    assert_eq!(parsed.exit_code, Some(101));
    assert!(!parsed.timed_out);
    assert_eq!(parsed.stdout_summary.as_deref(), Some("out-line-1"));
    assert_eq!(parsed.stderr_summary.as_deref(), Some("err-line-1"));
    assert_eq!(parsed.output_path.as_deref(), Some("/tmp/log"));
    assert_eq!(parsed.timeout_secs, 30);

    let serialized = serde_json::to_string(&parsed).expect("serialize GateResult");
    // The serialized JSON must use exactly this canonical, closed key set. A
    // rename (e.g. command_line → commandLine) and an *unannounced addition*
    // (e.g. a new `pub field: T`) are both wire-shape drift that downstream
    // proof/state consumers cannot absorb silently — assert key set equality,
    // not just presence.
    let value: serde_json::Value =
        serde_json::from_str(&serialized).expect("serialized JSON must reparse");
    let actual_keys: std::collections::BTreeSet<&str> = value
        .as_object()
        .expect("GateResult must serialize as a JSON object")
        .keys()
        .map(String::as_str)
        .collect();
    let expected_keys: std::collections::BTreeSet<&str> = [
        "circuit_breaker_open",
        "command_line",
        "duration_ms",
        "exit_code",
        "name",
        "output_path",
        "passed",
        "required",
        "stderr",
        "stderr_summary",
        "stdout",
        "stdout_summary",
        "timed_out",
        "timeout_secs",
    ]
    .into_iter()
    .collect();
    assert_eq!(
        actual_keys, expected_keys,
        "GateResult on-disk key set must stay closed; any rename or addition breaks proof/state consumers",
    );
}

#[tokio::test]
async fn test_gate_result_deserializes_with_legacy_minimal_fields() {
    let json = r#"{
        "name": "legacy",
        "passed": true,
        "stdout": "",
        "stderr": "",
        "duration_ms": 17,
        "required": true
    }"#;
    let parsed: omk::runtime::gates::GateResult =
        serde_json::from_str(json).expect("legacy GateResult JSON must deserialize");
    assert_eq!(parsed.name, "legacy");
    assert!(parsed.passed);
    assert!(parsed.required);
    assert_eq!(parsed.duration_ms, 17);
    assert_eq!(parsed.command_line, "");
    assert_eq!(parsed.exit_code, None);
    assert!(!parsed.timed_out);
    assert!(parsed.stdout_summary.is_none());
    assert!(parsed.stderr_summary.is_none());
    assert!(parsed.output_path.is_none());
    assert_eq!(parsed.timeout_secs, 0);
}

#[cfg(unix)]
#[tokio::test]
async fn test_run_gates_summary_truncates_to_three_lines_with_overflow_marker() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    let script = dir.join("many-lines.sh");
    write_unix_script(
        &script,
        "#!/bin/sh\nfor i in 1 2 3 4 5; do echo \"out-$i\"; done\nfor i in 1 2 3 4 5; do echo \"err-$i\" >&2; done\nexit 1\n",
    );

    let config = single_gate_config("many-lines", script.to_str().unwrap(), vec![], true, 30);

    let results = omk::runtime::gates::run_gates(&config, dir).await;
    assert_eq!(results.len(), 1);
    let g = &results[0];
    assert!(!g.passed);
    assert_eq!(g.exit_code, Some(1));

    let stdout_summary = g
        .stdout_summary
        .as_deref()
        .expect("stdout_summary must be set");
    let stdout_lines: Vec<&str> = stdout_summary.lines().collect();
    assert_eq!(
        stdout_lines,
        vec!["out-1", "out-2", "out-3", "..."],
        "stdout summary must keep first 3 lines plus '...' overflow marker",
    );

    let stderr_summary = g
        .stderr_summary
        .as_deref()
        .expect("stderr_summary must be set");
    let stderr_lines: Vec<&str> = stderr_summary.lines().collect();
    assert_eq!(
        stderr_lines,
        vec!["err-1", "err-2", "err-3", "..."],
        "stderr summary must keep first 3 lines plus '...' overflow marker",
    );

    assert!(
        g.stdout.lines().count() >= 5,
        "raw stdout must keep all 5 lines verbatim, got {} lines",
        g.stdout.lines().count(),
    );
    assert!(
        g.stderr.lines().count() >= 5,
        "raw stderr must keep all 5 lines verbatim, got {} lines",
        g.stderr.lines().count(),
    );
}

#[cfg(unix)]
#[tokio::test]
async fn test_run_gates_summary_truncates_long_lines_to_240_chars() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    let script = dir.join("long-line.sh");
    // 300 'x' characters on one line — well past the documented 240-char cap.
    // The script must not depend on `seq`/`printf` subprocesses so it stays
    // deterministic when many tests spawn children in parallel.
    let body = format!("#!/bin/sh\necho '{}'\nexit 0\n", "x".repeat(300));
    write_unix_script(&script, &body);

    let config = single_gate_config(
        "long-line",
        "/bin/sh",
        vec![script.to_string_lossy().into_owned()],
        true,
        5,
    );

    let results = omk::runtime::gates::run_gates(&config, dir).await;
    assert_eq!(results.len(), 1);
    let g = &results[0];
    assert!(
        g.passed,
        "long-line gate must succeed; exit_code={:?}, stderr={:?}",
        g.exit_code, g.stderr,
    );
    let summary = g
        .stdout_summary
        .as_deref()
        .expect("stdout_summary must be set");
    let first_line = summary.lines().next().expect("summary has at least 1 line");
    assert!(
        first_line.ends_with("..."),
        "summary line longer than 240 chars must end with '...': {first_line:?}",
    );
    assert_eq!(
        first_line.chars().count(),
        243,
        "summary line cap is 240 chars + trailing '...' (3 chars)",
    );
    assert!(
        g.stdout.lines().next().unwrap().chars().count() >= 300,
        "raw stdout must keep the full 300-char line verbatim",
    );
}
