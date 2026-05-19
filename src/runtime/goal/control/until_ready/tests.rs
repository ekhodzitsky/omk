use crate::runtime::gates::GateResult;
use crate::runtime::goal::control::until_ready::{
    manual_integration_acceptance_required, proof_can_continue, review_wall_blocker,
    terminal_blocker, verification_blocker, verification_can_continue, verification_summary,
};
use crate::runtime::goal::proof::GoalProof;
use crate::runtime::goal::state::GoalStatus;
use crate::runtime::goal::task_graph::{
    GoalTask, GoalTaskGraph, GoalTaskGraphSummary, GoalTaskStatus,
};
use chrono::Utc;

fn gate(name: &str, passed: bool) -> GateResult {
    GateResult {
        name: name.to_string(),
        passed,
        stdout: String::new(),
        stderr: String::new(),
        duration_ms: 0,
        required: true,
        command_line: String::new(),
        exit_code: None,
        timed_out: false,
        stdout_summary: None,
        stderr_summary: None,
        output_path: None,
        timeout_secs: 0,
    }
}

fn proof_with_gates(gates: Vec<GateResult>) -> GoalProof {
    GoalProof {
        version: 1,
        goal_id: "g1".to_string(),
        status: GoalStatus::Running,
        readiness: "not_ready".to_string(),
        summary: String::new(),
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
        gates,
        post_mutation_gates_ran: false,
        known_gaps: vec![],
        human_decisions_required: vec![],
        recovery_status: None,
    }
}

fn task(id: &str, status: GoalTaskStatus) -> GoalTask {
    GoalTask {
        id: id.to_string(),
        title: id.to_string(),
        description: String::new(),
        status,
        owner_role: None,
        completed_at: None,
        evidence: vec![],
        retry_count: 0,
        max_retries: 0,
        lease_expires_at: None,
        dependencies: vec![],
        read_set: vec![],
        write_set: vec![],
        risk: String::new(),
        acceptance: vec![],
    }
}

#[test]
fn verification_summary_empty_gates() {
    let proof = proof_with_gates(vec![]);
    assert_eq!(
        verification_summary(&proof),
        "no verification gates were detected or configured"
    );
}

#[test]
fn verification_summary_with_passed_gates() {
    let proof = proof_with_gates(vec![gate("lint", true), gate("test", false)]);
    assert_eq!(
        verification_summary(&proof),
        "ran 2 verification gate(s), 1 passed"
    );
}

#[test]
fn verification_can_continue_true_when_all_passed() {
    let proof = proof_with_gates(vec![gate("lint", true)]);
    assert!(verification_can_continue(&proof));
}

#[test]
fn verification_can_continue_false_when_empty_gates() {
    let proof = proof_with_gates(vec![]);
    assert!(!verification_can_continue(&proof));
}

#[test]
fn proof_can_continue_true_when_running_and_gates_pass() {
    let mut proof = proof_with_gates(vec![gate("lint", true)]);
    proof.status = GoalStatus::Running;
    assert!(proof_can_continue(&proof));
}

#[test]
fn proof_can_continue_false_when_status_not_continue() {
    let mut proof = proof_with_gates(vec![gate("lint", true)]);
    proof.status = GoalStatus::Ready;
    assert!(!proof_can_continue(&proof));
}

#[test]
fn proof_can_continue_false_when_gates_fail() {
    let mut proof = proof_with_gates(vec![gate("lint", false)]);
    proof.status = GoalStatus::Running;
    assert!(!proof_can_continue(&proof));
}

#[test]
fn verification_blocker_empty_gates() {
    let proof = proof_with_gates(vec![]);
    assert_eq!(
        verification_blocker(&proof),
        "verification blocked: no local gates were detected or configured"
    );
}

#[test]
fn verification_blocker_failed_gates() {
    let proof = proof_with_gates(vec![gate("lint", false), gate("test", true)]);
    assert_eq!(
        verification_blocker(&proof),
        "verification blocked: required gate(s) failed: lint"
    );
}

#[test]
fn terminal_blocker_needs_more_budget() {
    let proof = GoalProof {
        status: GoalStatus::NeedsMoreBudget,
        ..proof_with_gates(vec![])
    };
    let blocker = terminal_blocker(&proof);
    assert!(blocker.reason.contains("budget exhausted"));
}

#[test]
fn terminal_blocker_paused() {
    let proof = GoalProof {
        status: GoalStatus::Paused,
        ..proof_with_gates(vec![])
    };
    let blocker = terminal_blocker(&proof);
    assert!(blocker.reason.contains("paused"));
}

#[test]
fn terminal_blocker_cancelled() {
    let proof = GoalProof {
        status: GoalStatus::Cancelled,
        ..proof_with_gates(vec![])
    };
    let blocker = terminal_blocker(&proof);
    assert!(blocker.reason.contains("cancelled"));
}

#[test]
fn terminal_blocker_blocked_on_human() {
    let proof = GoalProof {
        status: GoalStatus::BlockedOnHuman,
        human_decisions_required: vec!["need approval".to_string()],
        ..proof_with_gates(vec![])
    };
    let blocker = terminal_blocker(&proof);
    assert!(blocker.reason.contains("need approval"));
    assert!(blocker.human_decision_required);
}

#[test]
fn terminal_blocker_blocked_on_external() {
    let proof = GoalProof {
        status: GoalStatus::BlockedOnExternal,
        ..proof_with_gates(vec![])
    };
    let blocker = terminal_blocker(&proof);
    assert!(blocker.reason.contains("external dependency"));
}

#[test]
fn terminal_blocker_failed_infra() {
    let proof = GoalProof {
        status: GoalStatus::FailedInfra,
        ..proof_with_gates(vec![])
    };
    let blocker = terminal_blocker(&proof);
    assert!(blocker.reason.contains("infrastructure failure"));
}

#[test]
fn terminal_blocker_fallback_to_verification() {
    let proof = proof_with_gates(vec![gate("lint", false)]);
    let blocker = terminal_blocker(&proof);
    assert!(blocker.reason.contains("lint"));
}

#[test]
fn review_wall_blocker_finds_blocked_review() {
    let proof = GoalProof {
        known_gaps: vec!["review is blocked on security".to_string()],
        ..proof_with_gates(vec![])
    };
    assert_eq!(
        review_wall_blocker(&proof),
        Some("review is blocked on security".to_string())
    );
}

#[test]
fn review_wall_blocker_finds_artifact() {
    let proof = GoalProof {
        known_gaps: vec!["missing review artifact for slice-1".to_string()],
        ..proof_with_gates(vec![])
    };
    assert_eq!(
        review_wall_blocker(&proof),
        Some("missing review artifact for slice-1".to_string())
    );
}

#[test]
fn review_wall_blocker_returns_none_when_no_match() {
    let proof = GoalProof {
        known_gaps: vec!["just a regular gap".to_string()],
        ..proof_with_gates(vec![])
    };
    assert_eq!(review_wall_blocker(&proof), None);
}

#[test]
fn manual_integration_acceptance_required_all_conditions_met() {
    let task_graph = GoalTaskGraph {
        version: 1,
        goal_id: "g1".to_string(),
        generated_at: Utc::now(),
        tasks: vec![
            task(
                crate::runtime::goal::state::GOAL_LOCAL_VERIFY_TASK_ID,
                GoalTaskStatus::Done,
            ),
            task(
                crate::runtime::goal::state::GOAL_AGENT_EXECUTE_TASK_ID,
                GoalTaskStatus::Done,
            ),
            task(
                crate::runtime::goal::state::GOAL_REVIEW_TASK_ID,
                GoalTaskStatus::Done,
            ),
            task(
                crate::runtime::goal::state::GOAL_SECURITY_REVIEW_TASK_ID,
                GoalTaskStatus::Done,
            ),
        ],
    };
    let proof = GoalProof {
        changed_files: vec!["a.rs".to_string()],
        post_mutation_gates_ran: true,
        ..proof_with_gates(vec![])
    };
    assert!(manual_integration_acceptance_required(&task_graph, &proof));
}

#[test]
fn manual_integration_acceptance_required_missing_execution() {
    let task_graph = GoalTaskGraph {
        version: 1,
        goal_id: "g1".to_string(),
        generated_at: Utc::now(),
        tasks: vec![
            task(
                crate::runtime::goal::state::GOAL_LOCAL_VERIFY_TASK_ID,
                GoalTaskStatus::Done,
            ),
            task(
                crate::runtime::goal::state::GOAL_AGENT_EXECUTE_TASK_ID,
                GoalTaskStatus::Pending,
            ),
        ],
    };
    let proof = GoalProof {
        changed_files: vec!["a.rs".to_string()],
        post_mutation_gates_ran: true,
        ..proof_with_gates(vec![])
    };
    assert!(!manual_integration_acceptance_required(&task_graph, &proof));
}

#[test]
fn manual_integration_acceptance_required_no_changed_files() {
    let task_graph = GoalTaskGraph {
        version: 1,
        goal_id: "g1".to_string(),
        generated_at: Utc::now(),
        tasks: vec![
            task(
                crate::runtime::goal::state::GOAL_LOCAL_VERIFY_TASK_ID,
                GoalTaskStatus::Done,
            ),
            task(
                crate::runtime::goal::state::GOAL_AGENT_EXECUTE_TASK_ID,
                GoalTaskStatus::Done,
            ),
            task(
                crate::runtime::goal::state::GOAL_REVIEW_TASK_ID,
                GoalTaskStatus::Done,
            ),
            task(
                crate::runtime::goal::state::GOAL_SECURITY_REVIEW_TASK_ID,
                GoalTaskStatus::Done,
            ),
        ],
    };
    let proof = GoalProof {
        changed_files: vec![],
        post_mutation_gates_ran: true,
        ..proof_with_gates(vec![])
    };
    assert!(!manual_integration_acceptance_required(&task_graph, &proof));
}

#[test]
fn manual_integration_acceptance_required_gates_not_ran() {
    let task_graph = GoalTaskGraph {
        version: 1,
        goal_id: "g1".to_string(),
        generated_at: Utc::now(),
        tasks: vec![
            task(
                crate::runtime::goal::state::GOAL_LOCAL_VERIFY_TASK_ID,
                GoalTaskStatus::Done,
            ),
            task(
                crate::runtime::goal::state::GOAL_AGENT_EXECUTE_TASK_ID,
                GoalTaskStatus::Done,
            ),
            task(
                crate::runtime::goal::state::GOAL_REVIEW_TASK_ID,
                GoalTaskStatus::Done,
            ),
            task(
                crate::runtime::goal::state::GOAL_SECURITY_REVIEW_TASK_ID,
                GoalTaskStatus::Done,
            ),
        ],
    };
    let proof = GoalProof {
        changed_files: vec!["a.rs".to_string()],
        post_mutation_gates_ran: false,
        ..proof_with_gates(vec![])
    };
    assert!(!manual_integration_acceptance_required(&task_graph, &proof));
}

#[test]
fn manual_integration_acceptance_required_missing_review() {
    let task_graph = GoalTaskGraph {
        version: 1,
        goal_id: "g1".to_string(),
        generated_at: Utc::now(),
        tasks: vec![
            task(
                crate::runtime::goal::state::GOAL_LOCAL_VERIFY_TASK_ID,
                GoalTaskStatus::Done,
            ),
            task(
                crate::runtime::goal::state::GOAL_AGENT_EXECUTE_TASK_ID,
                GoalTaskStatus::Done,
            ),
            task(
                crate::runtime::goal::state::GOAL_REVIEW_TASK_ID,
                GoalTaskStatus::Pending,
            ),
            task(
                crate::runtime::goal::state::GOAL_SECURITY_REVIEW_TASK_ID,
                GoalTaskStatus::Done,
            ),
        ],
    };
    let proof = GoalProof {
        changed_files: vec!["a.rs".to_string()],
        post_mutation_gates_ran: true,
        ..proof_with_gates(vec![])
    };
    assert!(!manual_integration_acceptance_required(&task_graph, &proof));
}
