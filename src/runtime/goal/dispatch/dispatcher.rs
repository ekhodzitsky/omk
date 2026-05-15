use anyhow::Result;
use chrono::{DateTime, Utc};
use std::path::Path;

use crate::runtime::goal::agent::GoalAgentDispatchPlan;
use crate::runtime::goal::evidence::GoalAgentRunEvidence;
use crate::runtime::goal::state::GoalState;
use crate::runtime::goal::task_graph::{GoalTask, GoalTaskGraph};

pub trait GoalDispatcher: Send {
    async fn execute_wave(
        &self,
        state: &GoalState,
        task_graph: &GoalTaskGraph,
        project_dir: &Path,
        started_at: DateTime<Utc>,
        dispatch: &GoalAgentDispatchPlan,
    ) -> Result<GoalAgentRunEvidence>;

    async fn append_execution_events(
        &self,
        state: &GoalState,
        task: &GoalTask,
        evidence: &GoalAgentRunEvidence,
    ) -> Result<()>;
}
