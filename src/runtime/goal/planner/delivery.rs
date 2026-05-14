use anyhow::Result;
use std::path::Path;

use super::super::state::GoalState;
use super::super::task_graph::{self, GoalTaskGraph};
use super::super::worktree::{materialize_goal_worktrees, GoalWorktreeMaterializeRequest};

pub(super) async fn materialize_delivery_slices(
    state: &GoalState,
    task_graph: &GoalTaskGraph,
    project_dir: &Path,
) -> Result<()> {
    let worktrees_root = state.state_dir.join("worktrees");
    let plan = task_graph::plan_goal_delivery_slices(&worktrees_root, task_graph)?;
    if plan.slices.is_empty() {
        return Ok(());
    }

    let task_ids = plan
        .slices
        .iter()
        .map(|slice| slice.task_id.clone())
        .collect();
    materialize_goal_worktrees(GoalWorktreeMaterializeRequest {
        repo_dir: project_dir.to_path_buf(),
        worktrees_root,
        goal_dir: Some(state.state_dir.clone()),
        goal_id: state.goal_id.clone(),
        task_ids,
        dry_run: false,
    })
    .await?;
    task_graph::record_goal_delivery_slice_plan(&state.state_dir, &plan).await
}
