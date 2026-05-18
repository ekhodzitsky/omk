use chrono::{DateTime, Utc};
use std::path::PathBuf;

use crate::runtime::goal::state::{
    GOAL_AGENT_EXECUTE_TASK_ID, GOAL_CONTROLLER_ACTOR, GOAL_LOCAL_VERIFY_TASK_ID,
    GOAL_PRD_FILE, GOAL_SECURITY_REVIEW_FILE, GOAL_SECURITY_REVIEW_TASK_ID,
    GOAL_TASK_GRAPH_FILE, GOAL_TECHNICAL_PLAN_FILE, GOAL_TEST_SPEC_FILE,
    GOAL_REVIEW_FILE, GOAL_REVIEW_TASK_ID, GOAL_ARTIFACTS_DIR, GOAL_AGENT_RUNS_DIR,
    GOAL_REVIEW_ARTIFACTS_DIR, GOAL_PROOF_FILE, GOAL_GATE_ARTIFACTS_DIR,
};
use crate::runtime::goal::task_graph::{GoalTask, GoalTaskEvidence, GoalTaskStatus};

pub(crate) fn scaffold_intake_task(generated_at: DateTime<Utc>) -> GoalTask {
    GoalTask {
        id: "goal-intake".to_string(),
        title: "Clarify durable goal intent".to_string(),
        description: "Preserve the original request, normalized goal, assumptions, and current execution boundary.".to_string(),
        status: GoalTaskStatus::Done,
        owner_role: Some(GOAL_CONTROLLER_ACTOR.to_string()),
        completed_at: Some(generated_at),
        evidence: vec![GoalTaskEvidence {
            kind: "artifact".to_string(),
            path: PathBuf::from(GOAL_PRD_FILE),
            summary: "Goal brief records the original and normalized goal.".to_string(),
        }],
        retry_count: 0,
        max_retries: 0,
        lease_expires_at: None,
        dependencies: Vec::new(),
        read_set: vec!["goal.json".to_string()],
        write_set: vec![GOAL_PRD_FILE.to_string()],
        risk: "low".to_string(),
        acceptance: vec![
            "goal brief exists".to_string(),
            "original and normalized goals are recorded".to_string(),
        ],
    }
}

pub(crate) fn scaffold_plan_task(generated_at: DateTime<Utc>) -> GoalTask {
    GoalTask {
        id: "goal-plan".to_string(),
        title: "Design controller work plan".to_string(),
        description: "Persist a technical plan and explicit verification expectations for the goal controller.".to_string(),
        status: GoalTaskStatus::Done,
        owner_role: Some(GOAL_CONTROLLER_ACTOR.to_string()),
        completed_at: Some(generated_at),
        evidence: vec![
            GoalTaskEvidence {
                kind: "artifact".to_string(),
                path: PathBuf::from(GOAL_TECHNICAL_PLAN_FILE),
                summary: "Technical plan records controller phases and execution boundary.".to_string(),
            },
            GoalTaskEvidence {
                kind: "artifact".to_string(),
                path: PathBuf::from(GOAL_TASK_GRAPH_FILE),
                summary: "Task graph records the first controller-owned work graph.".to_string(),
            },
            GoalTaskEvidence {
                kind: "artifact".to_string(),
                path: PathBuf::from(GOAL_TEST_SPEC_FILE),
                summary: "Test spec records readiness expectations.".to_string(),
            },
        ],
        retry_count: 0,
        max_retries: 0,
        lease_expires_at: None,
        dependencies: vec!["goal-intake".to_string()],
        read_set: vec![GOAL_PRD_FILE.to_string()],
        write_set: vec![
            GOAL_TECHNICAL_PLAN_FILE.to_string(),
            GOAL_TEST_SPEC_FILE.to_string(),
        ],
        risk: "medium".to_string(),
        acceptance: vec![
            "technical plan exists".to_string(),
            "test spec describes readiness gates".to_string(),
        ],
    }
}

pub(crate) fn scaffold_local_verify_task() -> GoalTask {
    GoalTask {
        id: GOAL_LOCAL_VERIFY_TASK_ID.to_string(),
        title: "Run local verification wall".to_string(),
        description:
            "Run configured local gates, capture gate evidence, and refresh the goal proof."
                .to_string(),
        status: GoalTaskStatus::Pending,
        owner_role: None,
        completed_at: None,
        evidence: Vec::new(),
        retry_count: 0,
        max_retries: 0,
        lease_expires_at: None,
        dependencies: vec!["goal-plan".to_string()],
        read_set: vec![
            GOAL_PRD_FILE.to_string(),
            GOAL_TECHNICAL_PLAN_FILE.to_string(),
            GOAL_TEST_SPEC_FILE.to_string(),
            ".omk/gates.toml".to_string(),
        ],
        write_set: vec![
            format!("{GOAL_ARTIFACTS_DIR}/{GOAL_GATE_ARTIFACTS_DIR}"),
            GOAL_TASK_GRAPH_FILE.to_string(),
            GOAL_PROOF_FILE.to_string(),
        ],
        risk: "medium".to_string(),
        acceptance: vec![
            "all required gates pass".to_string(),
            "proof cites gate evidence".to_string(),
        ],
    }
}

pub(crate) fn scaffold_agent_execute_task() -> GoalTask {
    GoalTask {
        id: GOAL_AGENT_EXECUTE_TASK_ID.to_string(),
        title: "Execute agent-owned implementation tasks".to_string(),
        description: "Run bounded agent work through Wire, allow minimal project mutations, and capture mutation evidence before readiness can be claimed.".to_string(),
        status: GoalTaskStatus::Pending,
        owner_role: None,
        completed_at: None,
        evidence: Vec::new(),
        retry_count: 0,
        max_retries: 0,
        lease_expires_at: None,
        dependencies: vec![GOAL_LOCAL_VERIFY_TASK_ID.to_string()],
        read_set: vec![
            GOAL_PRD_FILE.to_string(),
            GOAL_TECHNICAL_PLAN_FILE.to_string(),
            GOAL_TEST_SPEC_FILE.to_string(),
            GOAL_TASK_GRAPH_FILE.to_string(),
        ],
        write_set: vec![
            "project files".to_string(),
            GOAL_TASK_GRAPH_FILE.to_string(),
            GOAL_PROOF_FILE.to_string(),
        ],
        risk: "high".to_string(),
        acceptance: vec![
            "agent execution produces task and mutation evidence".to_string(),
            "proof status becomes ready only with evidence".to_string(),
        ],
    }
}

pub(crate) fn scaffold_review_task(agent_dependency: &str) -> GoalTask {
    GoalTask {
        id: GOAL_REVIEW_TASK_ID.to_string(),
        title: "Review goal execution evidence".to_string(),
        description: "Check that local verification and agent execution evidence are present before any readiness claim.".to_string(),
        status: GoalTaskStatus::Pending,
        owner_role: None,
        completed_at: None,
        evidence: Vec::new(),
        retry_count: 0,
        max_retries: 0,
        lease_expires_at: None,
        dependencies: vec![agent_dependency.to_string()],
        read_set: vec![
            GOAL_PRD_FILE.to_string(),
            GOAL_TECHNICAL_PLAN_FILE.to_string(),
            GOAL_TEST_SPEC_FILE.to_string(),
            GOAL_TASK_GRAPH_FILE.to_string(),
            GOAL_PROOF_FILE.to_string(),
            format!("{GOAL_ARTIFACTS_DIR}/{GOAL_AGENT_RUNS_DIR}"),
        ],
        write_set: vec![
            format!("{GOAL_ARTIFACTS_DIR}/{GOAL_REVIEW_ARTIFACTS_DIR}/{GOAL_REVIEW_FILE}"),
            GOAL_TASK_GRAPH_FILE.to_string(),
            GOAL_PROOF_FILE.to_string(),
        ],
        risk: "medium".to_string(),
        acceptance: vec![
            "review artifact cites task and gate evidence".to_string(),
            "review task remains blocked when execution evidence is missing".to_string(),
        ],
    }
}

pub(crate) fn scaffold_security_review_task(agent_dependency: &str) -> GoalTask {
    GoalTask {
        id: GOAL_SECURITY_REVIEW_TASK_ID.to_string(),
        title: "Run security evidence check".to_string(),
        description:
            "Run a bounded controller security review over goal evidence and changed files."
                .to_string(),
        status: GoalTaskStatus::Pending,
        owner_role: None,
        completed_at: None,
        evidence: Vec::new(),
        retry_count: 0,
        max_retries: 0,
        lease_expires_at: None,
        dependencies: vec![agent_dependency.to_string()],
        read_set: vec![
            GOAL_PROOF_FILE.to_string(),
            GOAL_TASK_GRAPH_FILE.to_string(),
            "changed files".to_string(),
        ],
        write_set: vec![
            format!("{GOAL_ARTIFACTS_DIR}/{GOAL_REVIEW_ARTIFACTS_DIR}/{GOAL_SECURITY_REVIEW_FILE}"),
            GOAL_TASK_GRAPH_FILE.to_string(),
            GOAL_PROOF_FILE.to_string(),
        ],
        risk: "high".to_string(),
        acceptance: vec![
            "security review artifact exists".to_string(),
            "high-confidence secret findings block the task".to_string(),
        ],
    }
}
