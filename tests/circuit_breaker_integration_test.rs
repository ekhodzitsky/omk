#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use omk::runtime::db::CircuitBreakerRepo;

#[cfg(unix)]
#[tokio::test]
async fn test_gate_skipped_when_circuit_open() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();

    // Write a script that always fails.
    let script = dir.join("fail.sh");
    std::fs::write(&script, "#!/bin/bash\necho 'error' >&2\nexit 1\n").unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();

    let config = omk::runtime::gates::VerificationConfig {
        gates: vec![omk::runtime::gates::GateDef {
            name: "always-fail".to_string(),
            command: script.to_string_lossy().to_string(),
            args: vec![],
            required: true,
            timeout_secs: 5,
            circuit_breaker: Some(omk::runtime::gates::CircuitBreakerConfig {
                failure_threshold: 3,
                recovery_timeout_secs: 3600,
                half_open_max_calls: 1,
                enabled: true,
            }),
        }],
    };

    // Run 3 times to open the circuit.
    for _ in 0..3 {
        let results = omk::runtime::gates::run_gates_with_evidence(&config, dir, None).await;
        assert_eq!(results.len(), 1);
        assert!(!results[0].passed);
        assert!(!results[0].circuit_breaker_open);
    }

    // 4th run: circuit should be open, gate skipped with synthetic failure.
    let results = omk::runtime::gates::run_gates_with_evidence(&config, dir, None).await;
    assert_eq!(results.len(), 1);
    assert!(!results[0].passed);
    assert!(results[0].circuit_breaker_open);
    assert_eq!(results[0].duration_ms, 0);
    assert!(results[0].stderr.contains("Circuit breaker OPEN"));
}

#[cfg(unix)]
#[tokio::test]
async fn test_gate_recovery_after_timeout() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();

    // Write a script that fails once then succeeds.
    let script = dir.join("fail_once.sh");
    let counter = dir.join(".counter");
    std::fs::write(
        &script,
        format!(
            "#!/bin/bash\nif [ -f {} ]; then\n  echo 'ok'\n  exit 0\nfi\ntouch {}\necho 'error' >&2\nexit 1\n",
            counter.display(),
            counter.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();

    let config = omk::runtime::gates::VerificationConfig {
        gates: vec![omk::runtime::gates::GateDef {
            name: "fail-once".to_string(),
            command: script.to_string_lossy().to_string(),
            args: vec![],
            required: true,
            timeout_secs: 5,
            circuit_breaker: Some(omk::runtime::gates::CircuitBreakerConfig {
                failure_threshold: 1,
                recovery_timeout_secs: 1,
                half_open_max_calls: 1,
                enabled: true,
            }),
        }],
    };

    // First run fails, circuit opens.
    let results = omk::runtime::gates::run_gates_with_evidence(&config, dir, None).await;
    assert_eq!(results.len(), 1);
    assert!(!results[0].passed);
    assert!(!results[0].circuit_breaker_open);

    // Second run: circuit open, skipped.
    let results = omk::runtime::gates::run_gates_with_evidence(&config, dir, None).await;
    assert_eq!(results.len(), 1);
    assert!(!results[0].passed);
    assert!(results[0].circuit_breaker_open);

    // Wait for recovery timeout.
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;

    // Third run: HalfOpen allows one probe. Script succeeds because counter file exists.
    let results = omk::runtime::gates::run_gates_with_evidence(&config, dir, None).await;
    assert_eq!(results.len(), 1);
    assert!(results[0].passed);
    assert!(!results[0].circuit_breaker_open);

    // Fourth run: circuit closed, normal execution.
    let results = omk::runtime::gates::run_gates_with_evidence(&config, dir, None).await;
    assert_eq!(results.len(), 1);
    assert!(results[0].passed);
    assert!(!results[0].circuit_breaker_open);
}

#[cfg(unix)]
#[tokio::test]
async fn test_gate_probe_failure_reopens() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();

    // Script that always fails.
    let script = dir.join("always_fail.sh");
    std::fs::write(&script, "#!/bin/bash\necho 'error' >&2\nexit 1\n").unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();

    let config = omk::runtime::gates::VerificationConfig {
        gates: vec![omk::runtime::gates::GateDef {
            name: "always-fail".to_string(),
            command: script.to_string_lossy().to_string(),
            args: vec![],
            required: true,
            timeout_secs: 5,
            circuit_breaker: Some(omk::runtime::gates::CircuitBreakerConfig {
                failure_threshold: 1,
                recovery_timeout_secs: 1,
                half_open_max_calls: 1,
                enabled: true,
            }),
        }],
    };

    // First run fails, circuit opens.
    let results = omk::runtime::gates::run_gates_with_evidence(&config, dir, None).await;
    assert!(!results[0].passed);

    // Wait for recovery timeout.
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;

    // Second run: probe in HalfOpen fails, circuit reopens.
    let results = omk::runtime::gates::run_gates_with_evidence(&config, dir, None).await;
    assert!(!results[0].passed);
    assert!(!results[0].circuit_breaker_open); // probe was attempted, not skipped

    // Third run: circuit open again, skipped.
    let results = omk::runtime::gates::run_gates_with_evidence(&config, dir, None).await;
    assert!(!results[0].passed);
    assert!(results[0].circuit_breaker_open);
}

#[tokio::test]
async fn test_sqlite_persistence() {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("cb_test.db");
    let db = omk::runtime::db::handle::DbHandle::open(&db_path)
        .await
        .unwrap();
    let repo = db.circuit_breaker_repo();

    let record = omk::runtime::db::repo::circuit_breaker::CircuitBreakerRecord {
        id: "/tmp/proj:lint".to_string(),
        gate_name: "lint".to_string(),
        project_path: "/tmp/proj".to_string(),
        state: "open".to_string(),
        consecutive_failures: 5,
        failure_threshold: 5,
        recovery_timeout_secs: 30,
        half_open_max_calls: 1,
        half_open_calls_remaining: 0,
        last_failure_at: Some(chrono::Utc::now()),
        last_success_at: None,
        opened_at: Some(chrono::Utc::now()),
        updated_at: chrono::Utc::now(),
    };

    repo.save(&record).await.unwrap();
    let loaded = repo.load_all().await.unwrap();
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].id, record.id);
    assert_eq!(loaded[0].state, "open");
    assert_eq!(loaded[0].consecutive_failures, 5);
}

#[tokio::test]
async fn test_parallel_check_thread_safe() {
    let registry = std::sync::Arc::new(omk::runtime::gates::CircuitBreakerRegistry::new());
    let path = Path::new("/tmp/parallel-test");

    let mut handles = Vec::new();
    for _ in 0..20 {
        let reg = registry.clone();
        let handle = tokio::spawn(async move {
            for _ in 0..50 {
                let _ = reg.check("lint", path, None).await;
                reg.record_failure("lint", path, false, false).await;
            }
        });
        handles.push(handle);
    }

    for h in handles {
        h.await.unwrap();
    }

    // Circuit should be open after 1000 failures.
    let check = registry.check("lint", path, None).await;
    assert!(
        matches!(
            check,
            omk::runtime::gates::circuit_breaker::CircuitCheck::Deny { .. }
        ),
        "Expected circuit to be open after parallel failures"
    );
}
