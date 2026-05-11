use crate::runtime::events::{EventBuilder, GateId, RunId, WorkerId};
use crate::runtime::gates::GateResult;
use crate::runtime::proof::{GateStatus, ProofGenerator, ProofStatus};

#[test]
fn proof_from_gate_results() {
    let gate_results = vec![
        GateResult {
            name: "fmt".to_string(),
            passed: true,
            stdout: "ok".to_string(),
            stderr: "".to_string(),
            duration_ms: 100,
            required: true,
            command_line: "cargo fmt --check".to_string(),
            exit_code: Some(0),
            timed_out: false,
            stdout_summary: Some("ok".to_string()),
            stderr_summary: Some(String::new()),
            output_path: Some("/tmp/gates/fmt.log".to_string()),
            timeout_secs: 120,
        },
        GateResult {
            name: "test".to_string(),
            passed: false,
            stdout: "failed".to_string(),
            stderr: "".to_string(),
            duration_ms: 200,
            required: true,
            command_line: "cargo test".to_string(),
            exit_code: Some(101),
            timed_out: false,
            stdout_summary: Some("failed".to_string()),
            stderr_summary: Some(String::new()),
            output_path: Some("/tmp/gates/test.log".to_string()),
            timeout_secs: 120,
        },
    ];

    let proof = ProofGenerator::from_gate_results(
        RunId("run-1".to_string()),
        &gate_results,
        &["src/main.rs".to_string()],
        &["docs".to_string()],
    );

    assert_eq!(proof.status, ProofStatus::Failed);
    assert_eq!(proof.gates.len(), 2);
    assert_eq!(proof.changed_files.len(), 1);
}

#[tokio::test]
async fn proof_from_events() {
    let tmp = tempfile::tempdir().unwrap();
    let event_log = tmp.path().join("events.jsonl");
    let writer = crate::runtime::events::EventWriter::new(&event_log);
    let run_id = RunId("run-test".to_string());
    let builder = EventBuilder::new(run_id.clone());

    let events = vec![
        builder.run_started("team", tmp.path(), "test").unwrap(),
        builder
            .worker_started(WorkerId("w1".to_string()), "coder")
            .unwrap(),
        builder.file_changed("src/main.rs", "modified").unwrap(),
        builder
            .gate_passed(GateId("g1".to_string()), "fmt", true)
            .unwrap(),
        builder
            .gate_failed(GateId("g2".to_string()), "test", true)
            .unwrap(),
        builder.run_completed(),
    ];

    for e in &events {
        writer.append(e).await.unwrap();
    }

    let proof = ProofGenerator::from_events(&run_id, &event_log)
        .await
        .unwrap();
    assert_eq!(proof.status, ProofStatus::Failed);
    assert_eq!(proof.changed_files.len(), 1);
    assert_eq!(proof.gates.len(), 2);
    assert_eq!(proof.gates[0].status, GateStatus::Passed);
    assert_eq!(proof.gates[1].status, GateStatus::Failed);
}
