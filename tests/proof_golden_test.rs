use tempfile::TempDir;

mod fixture_runner;
use fixture_runner::FixtureRunner;

#[tokio::test]
async fn test_proof_golden_happy_path() {
    let tmp = TempDir::new().unwrap();
    let runner = FixtureRunner::new(tmp.path(), "golden-happy");

    runner
        .emit_run_started("team", tmp.path(), "Implement feature X")
        .await;
    runner.emit_worker_started("worker-0", "coder").await;
    runner.emit_file_changed("src/feature.rs", "modified").await;
    runner.emit_gate_passed("fmt").await;
    runner.emit_gate_passed("clippy").await;
    runner.emit_gate_passed("test").await;
    runner
        .emit_task_completed("task-1", "worker-0", "Feature implemented")
        .await;
    runner.emit_run_completed().await;

    let proof = runner.generate_proof();
    assert_eq!(proof.status, omk::runtime::proof::ProofStatus::Ready);
    assert_eq!(proof.changed_files.len(), 1);
    assert_eq!(proof.gates.len(), 3);
    assert!(proof.failures.is_empty());
    assert_eq!(proof.readiness(), "ready_for_handoff");
    assert_eq!(
        proof.readiness_text(),
        "Ready for handoff: required gates passed and no blocking failures."
    );
    assert!(proof.summary.contains("ready"));
    let md = proof.to_markdown();
    assert!(md.contains("## Verdict"));
    assert!(md.contains("Readiness verdict: `ready_for_handoff`."));
}

#[tokio::test]
async fn test_proof_golden_with_failures() {
    let tmp = TempDir::new().unwrap();
    let runner = FixtureRunner::new(tmp.path(), "golden-failures");

    runner
        .emit_run_started("team", tmp.path(), "Fix failing tests")
        .await;
    runner.emit_worker_started("worker-0", "coder").await;
    runner.emit_file_changed("src/lib.rs", "modified").await;
    runner.emit_gate_passed("fmt").await;
    runner.emit_gate_failed("test", "3 tests failed").await;
    runner
        .emit_task_completed("task-1", "worker-0", "Partial fix")
        .await;
    runner.emit_run_completed().await;

    let proof = runner.generate_proof();
    // Required gate failure results in Failed status.
    assert_eq!(proof.status, omk::runtime::proof::ProofStatus::Failed);
    assert_eq!(proof.gates.len(), 2); // fmt + test
    assert_eq!(proof.failures.len(), 1);
    assert_eq!(proof.readiness(), "blocked");
    assert_eq!(
        proof.readiness_text(),
        "Blocked: failures or required gate failures must be resolved."
    );
    assert!(proof.known_gaps.contains(&"gate test failed".to_string()));
    let md = proof.to_markdown();
    assert!(md.contains("Readiness verdict: `blocked`."));
}

#[tokio::test]
async fn test_proof_golden_empty_run() {
    let tmp = TempDir::new().unwrap();
    let runner = FixtureRunner::new(tmp.path(), "golden-empty");

    runner
        .emit_run_started("team", tmp.path(), "Refactor")
        .await;
    runner.emit_run_completed().await;

    let proof = runner.generate_proof();
    assert_eq!(proof.status, omk::runtime::proof::ProofStatus::NotReady);
    assert_eq!(proof.readiness(), "needs_follow_up");
    assert_eq!(
        proof.readiness_text(),
        "Needs follow-up: required gates are incomplete or missing."
    );
    assert!(proof.changed_files.is_empty());
    assert!(proof.gates.is_empty());
}

#[tokio::test]
async fn test_proof_from_gate_results_direct() {
    use omk::runtime::gates::GateResult;
    use omk::runtime::proof::{ProofGenerator, ProofStatus};

    let gates = vec![
        GateResult {
            name: "fmt".to_string(),
            passed: true,
            stdout: "ok".to_string(),
            stderr: String::new(),
            duration_ms: 100,
            required: true,
        },
        GateResult {
            name: "clippy".to_string(),
            passed: true,
            stdout: "ok".to_string(),
            stderr: String::new(),
            duration_ms: 200,
            required: true,
        },
        GateResult {
            name: "test".to_string(),
            passed: false,
            stdout: String::new(),
            stderr: "1 test failed".to_string(),
            duration_ms: 500,
            required: true,
        },
    ];

    let proof = ProofGenerator::from_gate_results(
        omk::runtime::events::RunId("direct-run".to_string()),
        &gates,
        &["src/lib.rs".to_string()],
        &[],
    );
    // Required gate failure results in Failed status.
    assert_eq!(proof.status, ProofStatus::Failed);
    assert_eq!(proof.gates.len(), 3);
    assert_eq!(proof.changed_files.len(), 1);
}
