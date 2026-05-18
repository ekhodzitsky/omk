use super::{reconcile_goal_proof_with_state, GoalProof};
use crate::runtime::gates::GateResult;
use crate::runtime::goal::state::{GoalState, GoalStatus};
use crate::runtime::goal::task_graph::GoalTaskGraphSummary;
use chrono::Utc;
use std::path::Path;

fn fixture_path(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/proof")
        .join(name)
}

fn load_proof(name: &str) -> GoalProof {
    let path = fixture_path(name);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read proof fixture {}: {e}", path.display()));
    serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("failed to parse proof fixture {}: {e}", path.display()))
}

fn load_state(name: &str) -> GoalState {
    let path = fixture_path(name);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read state fixture {}: {e}", path.display()));
    serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("failed to parse state fixture {}: {e}", path.display()))
}

fn load_json(name: &str) -> serde_json::Value {
    let path = fixture_path(name);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read fixture {}: {e}", path.display()));
    serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("failed to parse fixture {}: {e}", path.display()))
}

#[test]
fn reconcile_goal_proof_with_running_state_is_no_op() {
    let mut proof = load_proof("proof_before.json");
    let state = load_state("goal_state_running.json");
    reconcile_goal_proof_with_state(&mut proof, &state);
    let actual = serde_json::to_value(&proof).unwrap();
    let expected = load_json("proof_after_running.json");
    assert_eq!(
        actual, expected,
        "reconciling with Running state should not mutate proof"
    );
}

#[test]
fn reconcile_goal_proof_with_ready_state_is_no_op() {
    let mut proof = load_proof("proof_before.json");
    let state = load_state("goal_state_done.json");
    reconcile_goal_proof_with_state(&mut proof, &state);
    let actual = serde_json::to_value(&proof).unwrap();
    let expected = load_json("proof_after_done.json");
    assert_eq!(
        actual, expected,
        "reconciling with Ready state should not mutate proof"
    );
}

fn ready_proof() -> GoalProof {
    GoalProof {
        version: 1,
        goal_id: "g1".to_string(),
        status: GoalStatus::Ready,
        readiness: "ready".to_string(),
        summary: "summary".to_string(),
        generated_at: Utc::now(),
        artifacts: vec![],
        task_graph_summary: GoalTaskGraphSummary {
            total_tasks: 0,
            pending_tasks: 0,
            blocked_tasks: 0,
            done_tasks: 0,
        },
        changed_files: vec![],
        commits: vec![],
        git: None,
        gates: vec![GateResult {
            name: "test".to_string(),
            passed: true,
            stdout: String::new(),
            stderr: String::new(),
            duration_ms: 0,
            required: true,
            command_line: String::new(),
            exit_code: Some(0),
            timed_out: false,
            stdout_summary: None,
            stderr_summary: None,
            output_path: None,
            timeout_secs: 0,
        }],
        post_mutation_gates_ran: true,
        known_gaps: vec![],
        human_decisions_required: vec![],
        recovery_status: None,
    }
}

#[test]
fn validate_for_merge_blocks_when_proof_status_not_ready() {
    let mut proof = ready_proof();
    proof.status = GoalStatus::NotReady;
    let err = proof.validate_for_merge().unwrap_err().to_string();
    assert!(
        err.contains("not Ready"),
        "expected not-Ready error, got: {err}"
    );
}

#[test]
fn validate_for_merge_blocks_when_gates_fail() {
    let mut proof = ready_proof();
    proof.gates = vec![GateResult {
        name: "test".to_string(),
        passed: false,
        stdout: String::new(),
        stderr: String::new(),
        duration_ms: 0,
        required: true,
        command_line: String::new(),
        exit_code: Some(1),
        timed_out: false,
        stdout_summary: None,
        stderr_summary: None,
        output_path: None,
        timeout_secs: 0,
    }];
    let err = proof.validate_for_merge().unwrap_err().to_string();
    assert!(err.contains("gates"), "expected gates error, got: {err}");
}

#[test]
fn validate_for_merge_blocks_when_review_wall_fails() {
    let proof = ready_proof();
    let artifacts = vec![serde_json::json!({
        "pass": "security",
        "status": "blocked",
        "path": "review.json",
        "summary": "blocked",
        "evidence": [],
        "risks": [],
        "known_gaps": ["gap"],
        "recommended_next_step": "fix",
    })];
    super::sidecar::remember_goal_proof_review_artifacts(&proof, artifacts);
    let err = proof.validate_for_merge().unwrap_err().to_string();
    assert!(
        err.contains("review wall"),
        "expected review wall error, got: {err}"
    );
}

#[test]
fn validate_for_merge_passes_when_all_checks_ok() {
    let proof = ready_proof();
    let artifacts = vec![serde_json::json!({
        "pass": "security",
        "status": "passed",
        "path": "review.json",
        "summary": "passed",
        "evidence": [],
        "risks": [],
        "known_gaps": [],
        "recommended_next_step": "ship",
    })];
    super::sidecar::remember_goal_proof_review_artifacts(&proof, artifacts);
    proof
        .validate_for_merge()
        .expect("should pass when proof is Ready, gates pass, and review wall passes");
}
