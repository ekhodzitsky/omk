use anyhow::Result;
use chrono::{DateTime, Utc};
use std::path::Path;

use super::dispatcher::GoalDispatcher;
use crate::runtime::goal::agent::GoalAgentDispatchPlan;
use crate::runtime::goal::dispatch::tasks::{
    append_agent_execution_task_events, run_goal_agent_task_wave,
};
use crate::runtime::goal::evidence::GoalAgentRunEvidence;
use crate::runtime::goal::state::GoalState;
use crate::runtime::goal::task_graph::{GoalTask, GoalTaskGraph};

pub struct DefaultGoalDispatcher;

impl GoalDispatcher for DefaultGoalDispatcher {
    async fn execute_wave(
        &self,
        state: &GoalState,
        task_graph: &GoalTaskGraph,
        project_dir: &Path,
        started_at: DateTime<Utc>,
        dispatch: &GoalAgentDispatchPlan,
    ) -> Result<GoalAgentRunEvidence> {
        run_goal_agent_task_wave(state, task_graph, project_dir, started_at, dispatch).await
    }

    async fn append_execution_events(
        &self,
        state: &GoalState,
        task: &GoalTask,
        evidence: &GoalAgentRunEvidence,
    ) -> Result<()> {
        append_agent_execution_task_events(state, task, evidence).await
    }
}
