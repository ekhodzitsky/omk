use super::*;
use chrono::Utc;

fn state() -> GoalState {
    GoalState {
        version: 1,
        goal_id: "goal-test".to_string(),
        original_goal: "Build a safe goal runtime".to_string(),
        normalized_goal: "Build a safe goal runtime".to_string(),
        status: super::super::state::GoalStatus::Running,
        phase: super::super::state::GoalPhase::Execution,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        completed_at: None,
        until_ready: true,
        budget_time: None,
        max_agents: Some(2),
        terminal_criteria: super::super::state::GoalTerminalCriteria::default(),
        artifacts: Vec::new(),
        failure: None,
        state_dir: std::path::PathBuf::from("/tmp/goal-test"),
    }
}

fn done_task(id: &str) -> super::super::task_graph::GoalTask {
    super::super::task_graph::GoalTask {
        id: id.to_string(),
        title: format!("Task {id}"),
        description: format!("Task {id} description"),
        status: GoalTaskStatus::Done,
        owner_role: None,
        completed_at: Some(Utc::now()),
        evidence: Vec::new(),
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
    GoalAgentTaskProposal {
        id: id.to_string(),
        title: format!("Task {id}"),
        description: format!("Task {id} description"),
        dependencies: dependencies
            .iter()
            .map(|dependency| dependency.to_string())
            .collect(),
        read_set: Vec::new(),
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
