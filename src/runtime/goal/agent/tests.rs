use super::*;
use chrono::Utc;

fn state() -> GoalState {
    GoalState {
        version: 1,
        goal_id: "goal-test".to_string(),
        original_goal: "Build a safe goal runtime".to_string(),
        normalized_goal: "Build a safe goal runtime".to_string(),
        status: crate::runtime::goal::state::GoalStatus::Running,
        phase: crate::runtime::goal::state::GoalPhase::Execution,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        completed_at: None,
        until_ready: true,
        budget_time: None,
        budget_tokens: None,
        budget_usd: None,
        max_agents: Some(2),
        terminal_criteria: crate::runtime::goal::state::GoalTerminalCriteria::default(),
        artifacts: Vec::new(),
        failure: None,
        state_dir: std::path::PathBuf::from("/tmp/goal-test"),
        cost_tracker_path: None,
    }
}

fn done_task(id: &str) -> crate::runtime::goal::task_graph::GoalTask {
    crate::runtime::goal::task_graph::GoalTask {
        id: id.to_string(),
        title: format!("Task {id}"),
        description: format!("Task {id} description"),
        status: GoalTaskStatus::Done,
        owner_role: None,
        completed_at: Some(Utc::now()),
        evidence: Vec::new(),
        retry_count: 0,
        max_retries: 0,
        lease_expires_at: None,
        dependencies: Vec::new(),
        read_set: Vec::new(),
        write_set: Vec::new(),
        risk: "low".to_string(),
        acceptance: vec![format!("Task {id} acceptance")],
    }
}

fn graph() -> GoalTaskGraph {
    GoalTaskGraph {
        version: 1,
        goal_id: "goal-test".to_string(),
        generated_at: Utc::now(),
        tasks: vec![done_task(GOAL_AGENT_EXECUTE_TASK_ID)],
    }
}

fn proposal(id: &str, dependencies: &[&str], write_set: &[&str]) -> GoalAgentTaskProposal {
    proposal_with_sets(id, dependencies, &[], write_set)
}

fn proposal_with_sets(
    id: &str,
    dependencies: &[&str],
    read_set: &[&str],
    write_set: &[&str],
) -> GoalAgentTaskProposal {
    GoalAgentTaskProposal {
        id: id.to_string(),
        title: format!("Task {id}"),
        description: format!("Task {id} description"),
        dependencies: dependencies
            .iter()
            .map(|dependency| dependency.to_string())
            .collect(),
        read_set: read_set.iter().map(|path| path.to_string()).collect(),
        write_set: write_set.iter().map(|path| path.to_string()).collect(),
        risk: "low".to_string(),
        acceptance: vec![format!("Task {id} acceptance")],
        budget_secs: 120,
        priority: 0,
    }
}

#[test]
fn policy_accepts_dependency_ordered_tasks_with_shared_write_set() {
    let policy = validate_goal_agent_task_proposals(
        &state(),
        &graph(),
        "goal-test-followups",
        vec![
            proposal(
                "goal-agent-docs-a",
                &[GOAL_AGENT_EXECUTE_TASK_ID],
                &["README.md"],
            ),
            proposal(
                "goal-agent-docs-b",
                &[GOAL_AGENT_EXECUTE_TASK_ID, "goal-agent-docs-a"],
                &["README.md"],
            ),
        ],
        false,
    );

    assert_eq!(policy.accepted_tasks.len(), 2);
    assert!(policy.rejected_tasks.is_empty());
}

#[test]
fn policy_accepts_dependency_ordered_read_after_write() {
    let policy = validate_goal_agent_task_proposals(
        &state(),
        &graph(),
        "goal-test-followups",
        vec![
            proposal_with_sets(
                "goal-agent-docs-a",
                &[GOAL_AGENT_EXECUTE_TASK_ID],
                &[],
                &["README.md"],
            ),
            proposal_with_sets(
                "goal-agent-docs-b",
                &[GOAL_AGENT_EXECUTE_TASK_ID, "goal-agent-docs-a"],
                &["README.md"],
                &["docs/guide.md"],
            ),
        ],
        false,
    );

    assert_eq!(policy.accepted_tasks.len(), 2);
    assert!(policy.rejected_tasks.is_empty());
}

#[test]
fn policy_accepts_parallel_tasks_with_shared_read_set() {
    let policy = validate_goal_agent_task_proposals(
        &state(),
        &graph(),
        "goal-test-followups",
        vec![
            proposal_with_sets(
                "goal-agent-docs-a",
                &[GOAL_AGENT_EXECUTE_TASK_ID],
                &["README.md"],
                &["docs/a.md"],
            ),
            proposal_with_sets(
                "goal-agent-docs-b",
                &[GOAL_AGENT_EXECUTE_TASK_ID],
                &["./README.md"],
                &["docs/b.md"],
            ),
        ],
        false,
    );

    assert_eq!(policy.accepted_tasks.len(), 2);
    assert!(policy.rejected_tasks.is_empty());
}

#[test]
fn policy_rejects_unordered_tasks_with_conflicting_write_set() {
    let policy = validate_goal_agent_task_proposals(
        &state(),
        &graph(),
        "goal-test-followups",
        vec![
            proposal(
                "goal-agent-docs-a",
                &[GOAL_AGENT_EXECUTE_TASK_ID],
                &["README.md"],
            ),
            proposal(
                "goal-agent-docs-b",
                &[GOAL_AGENT_EXECUTE_TASK_ID],
                &["README.md"],
            ),
        ],
        false,
    );

    assert_eq!(policy.accepted_tasks.len(), 1);
    assert_eq!(policy.rejected_tasks.len(), 1);
    assert_eq!(policy.rejected_tasks[0].task.id, "goal-agent-docs-b");
    assert!(policy.rejected_tasks[0]
        .reason
        .contains("write-set conflict with accepted task goal-agent-docs-a: README.md"));
}

#[test]
fn policy_rejects_unordered_tasks_with_normalized_write_path_conflict() {
    let policy = validate_goal_agent_task_proposals(
        &state(),
        &graph(),
        "goal-test-followups",
        vec![
            proposal(
                "goal-agent-docs-a",
                &[GOAL_AGENT_EXECUTE_TASK_ID],
                &["./README.md"],
            ),
            proposal(
                "goal-agent-docs-b",
                &[GOAL_AGENT_EXECUTE_TASK_ID],
                &["README.md"],
            ),
        ],
        false,
    );

    assert_eq!(policy.accepted_tasks.len(), 1);
    assert_eq!(policy.rejected_tasks.len(), 1);
    assert!(policy.rejected_tasks[0]
        .reason
        .contains("write-set conflict with accepted task goal-agent-docs-a: README.md"));
}

#[test]
fn policy_rejects_unordered_tasks_with_parent_child_write_path_conflict() {
    let policy = validate_goal_agent_task_proposals(
        &state(),
        &graph(),
        "goal-test-followups",
        vec![
            proposal(
                "goal-agent-docs-a",
                &[GOAL_AGENT_EXECUTE_TASK_ID],
                &["docs"],
            ),
            proposal(
                "goal-agent-docs-b",
                &[GOAL_AGENT_EXECUTE_TASK_ID],
                &["docs/guide.md"],
            ),
        ],
        false,
    );

    assert_eq!(policy.accepted_tasks.len(), 1);
    assert_eq!(policy.rejected_tasks.len(), 1);
    assert!(policy.rejected_tasks[0]
        .reason
        .contains("write-set conflict with accepted task goal-agent-docs-a: docs/guide.md"));
}

#[test]
fn policy_rejects_unordered_task_that_reads_accepted_write_set() {
    let policy = validate_goal_agent_task_proposals(
        &state(),
        &graph(),
        "goal-test-followups",
        vec![
            proposal_with_sets(
                "goal-agent-docs-a",
                &[GOAL_AGENT_EXECUTE_TASK_ID],
                &[],
                &["README.md"],
            ),
            proposal_with_sets(
                "goal-agent-docs-b",
                &[GOAL_AGENT_EXECUTE_TASK_ID],
                &["./README.md"],
                &["docs/guide.md"],
            ),
        ],
        false,
    );

    assert_eq!(policy.accepted_tasks.len(), 1);
    assert_eq!(policy.rejected_tasks.len(), 1);
    assert!(policy.rejected_tasks[0]
        .reason
        .contains("read/write conflict with accepted task goal-agent-docs-a: README.md"));
}

#[test]
fn policy_rejects_unordered_task_that_writes_accepted_read_set() {
    let policy = validate_goal_agent_task_proposals(
        &state(),
        &graph(),
        "goal-test-followups",
        vec![
            proposal_with_sets(
                "goal-agent-docs-a",
                &[GOAL_AGENT_EXECUTE_TASK_ID],
                &["docs"],
                &["agent-output.md"],
            ),
            proposal_with_sets(
                "goal-agent-docs-b",
                &[GOAL_AGENT_EXECUTE_TASK_ID],
                &[],
                &["docs/guide.md"],
            ),
        ],
        false,
    );

    assert_eq!(policy.accepted_tasks.len(), 1);
    assert_eq!(policy.rejected_tasks.len(), 1);
    assert!(policy.rejected_tasks[0]
        .reason
        .contains("write/read conflict with accepted task goal-agent-docs-a: docs/guide.md"));
}
