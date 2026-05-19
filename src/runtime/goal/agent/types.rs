use crate::runtime::goal::state::{goal_agent_task_budget_secs, GoalState};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalAgentTaskProposal {
    pub id: String,
    pub title: String,
    pub description: String,
    #[serde(default)]
    pub dependencies: Vec<String>,
    #[serde(default)]
    pub read_set: Vec<String>,
    #[serde(default)]
    pub write_set: Vec<String>,
    #[serde(default = "default_goal_agent_task_risk")]
    pub risk: String,
    #[serde(default)]
    pub acceptance: Vec<String>,
    #[serde(default = "crate::runtime::goal::state::default_goal_agent_task_budget_secs")]
    pub budget_secs: u64,
    #[serde(default)]
    pub priority: i32,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct GoalAgentRejectedTask {
    pub(crate) task: GoalAgentTaskProposal,
    pub(crate) reason: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct GoalAgentTaskPolicy {
    pub(crate) goal_id: String,
    pub(crate) run_id: String,
    pub(crate) max_agents: usize,
    pub(crate) proposed_tasks: Vec<GoalAgentTaskProposal>,
    pub(crate) accepted_tasks: Vec<GoalAgentTaskProposal>,
    pub(crate) rejected_tasks: Vec<GoalAgentRejectedTask>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GoalAgentWaveKind {
    Initial,
    FollowUp,
}

#[derive(Debug, Clone)]
pub struct GoalAgentDispatchPlan {
    pub(crate) run_key: String,
    pub(crate) kind: GoalAgentWaveKind,
    pub(crate) proposals: Vec<GoalAgentTaskProposal>,
    pub(crate) allow_existing_task_ids: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct AcceptedGoalAgentAccessSet {
    pub(crate) task_id: String,
    pub(crate) read_set: Vec<String>,
    pub(crate) write_set: Vec<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct GoalAgentAccessConflict {
    pub(crate) kind: &'static str,
    pub(crate) path: String,
}

pub(crate) fn default_goal_agent_task_risk() -> String {
    "moderate".to_string()
}

pub(crate) fn propose_goal_agent_tasks(state: &GoalState) -> Vec<GoalAgentTaskProposal> {
    use crate::runtime::goal::state::{
        GOAL_AGENT_IMPLEMENT_TASK_ID, GOAL_AGENT_PUBLISH_TASK_ID, GOAL_AGENT_VERIFY_TASK_ID,
        GOAL_ARTIFACTS_DIR, GOAL_PRD_FILE, GOAL_PROOF_FILE, GOAL_TASK_GRAPH_FILE,
        GOAL_TECHNICAL_PLAN_FILE, GOAL_TEST_SPEC_FILE,
    };

    vec![
        GoalAgentTaskProposal {
            id: GOAL_AGENT_IMPLEMENT_TASK_ID.to_string(),
            title: "Implement bounded goal slice".to_string(),
            description: "Make one bounded in-repository project change that moves the goal forward, then summarize changed files and remaining verification needs.".to_string(),
            dependencies: Vec::new(),
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
            risk: "moderate".to_string(),
            acceptance: vec![
                "Make only a bounded project change needed to move the goal forward.".to_string(),
                "Do not commit, publish, or touch secrets.".to_string(),
                "Summarize changed files and verification still needed for production readiness.".to_string(),
            ],
            budget_secs: goal_agent_task_budget_secs(state, 900),
            priority: 20,
        },
        GoalAgentTaskProposal {
            id: GOAL_AGENT_VERIFY_TASK_ID.to_string(),
            title: "Verify agent wave follow-up".to_string(),
            description: "Inspect the bounded implementation result and summarize verification, review, or hardening follow-up that still blocks readiness.".to_string(),
            dependencies: vec![GOAL_AGENT_IMPLEMENT_TASK_ID.to_string()],
            read_set: vec![
                GOAL_PRD_FILE.to_string(),
                GOAL_TECHNICAL_PLAN_FILE.to_string(),
                GOAL_TEST_SPEC_FILE.to_string(),
                GOAL_TASK_GRAPH_FILE.to_string(),
                "project files".to_string(),
            ],
            write_set: vec![GOAL_ARTIFACTS_DIR.to_string()],
            risk: "low".to_string(),
            acceptance: vec![
                "Review the bounded project change and call out remaining verification gaps.".to_string(),
                "Do not make broad follow-up mutations without a new controller-approved task.".to_string(),
                "Keep the goal proof honest when production readiness is still blocked.".to_string(),
            ],
            budget_secs: goal_agent_task_budget_secs(state, 300),
            priority: 10,
        },
        GoalAgentTaskProposal {
            id: GOAL_AGENT_PUBLISH_TASK_ID.to_string(),
            title: "Publish crate release".to_string(),
            description: "Publish this crate to crates.io after the agent wave succeeds.".to_string(),
            dependencies: vec![GOAL_AGENT_VERIFY_TASK_ID.to_string()],
            read_set: vec![GOAL_PROOF_FILE.to_string()],
            write_set: vec!["crates.io".to_string()],
            risk: "external-side-effect".to_string(),
            acceptance: vec!["Publish the package to crates.io.".to_string()],
            budget_secs: goal_agent_task_budget_secs(state, 300),
            priority: 0,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::goal::state::GoalState;

    #[test]
    fn propose_goal_agent_tasks_returns_three_tasks() {
        let state = GoalState {
            version: 1,
            goal_id: "g1".to_string(),
            original_goal: "test".to_string(),
            normalized_goal: "test goal".to_string(),
            status: crate::runtime::goal::state::GoalStatus::Running,
            phase: crate::runtime::goal::state::GoalPhase::Execution,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            completed_at: None,
            until_ready: false,
            budget_time: None,
            budget_tokens: None,
            budget_usd: None,
            max_agents: None,
            terminal_criteria: Default::default(),
            artifacts: vec![],
            failure: None,
            state_dir: std::path::PathBuf::from("/tmp/test"),
            cost_tracker_path: None,
            delivery_policy: Default::default(),
            merge_policy: Default::default(),
            slice_execution: false,
        };
        let tasks = propose_goal_agent_tasks(&state);
        assert_eq!(tasks.len(), 3);
        assert_eq!(
            tasks[0].id,
            crate::runtime::goal::state::GOAL_AGENT_IMPLEMENT_TASK_ID
        );
        assert_eq!(
            tasks[1].id,
            crate::runtime::goal::state::GOAL_AGENT_VERIFY_TASK_ID
        );
        assert_eq!(
            tasks[2].id,
            crate::runtime::goal::state::GOAL_AGENT_PUBLISH_TASK_ID
        );
    }

    #[test]
    fn default_goal_agent_task_risk_is_moderate() {
        assert_eq!(default_goal_agent_task_risk(), "moderate");
    }
}
