use std::path::Path;

use anyhow::Result;

use crate::runtime::goal::agent::GoalAgentTaskProposal;
use crate::runtime::goal::dispatch::tasks::results::{
    read_goal_agent_worker_results, summarize_goal_agent_worker_results,
};
use crate::runtime::goal::evidence::{
    extract_goal_agent_task_proposals, write_goal_agent_mutation_snapshot,
};
use crate::runtime::goal::state::GoalState;
use crate::runtime::scheduler::runner::RunSummary;
use crate::runtime::worker::{WorkerResult, WorkerSpec};

pub(crate) async fn gather_wave_results(
    worker_specs: &[WorkerSpec],
    accepted_task_ids: &[String],
    state: &GoalState,
    project_dir: &Path,
    mutation_diff_path: &Path,
    changed_files_path: &Path,
    summary: &RunSummary,
) -> Result<(
    Vec<WorkerResult>,
    Option<String>,
    Vec<GoalAgentTaskProposal>,
    Vec<String>,
)> {
    let worker_results = read_goal_agent_worker_results(worker_specs, accepted_task_ids).await?;
    let worker_summary = summarize_goal_agent_worker_results(&worker_results)
        .or_else(|| (summary.cancelled > 0).then(|| "cancelled by user".to_string()));
    let agent_proposed_tasks = extract_goal_agent_task_proposals(&worker_results);
    let changed_files = write_goal_agent_mutation_snapshot(
        state,
        project_dir,
        mutation_diff_path,
        changed_files_path,
    )
    .await?;
    Ok((
        worker_results,
        worker_summary,
        agent_proposed_tasks,
        changed_files,
    ))
}
