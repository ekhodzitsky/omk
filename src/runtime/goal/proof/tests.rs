use super::{reconcile_goal_proof_with_state, GoalProof};
use crate::runtime::goal::state::GoalState;
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
