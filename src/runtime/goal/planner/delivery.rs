use anyhow::Result;
use std::path::Path;

use crate::runtime::goal::state::GoalState;
use crate::runtime::goal::task_graph::{self, GoalTaskGraph};
use crate::runtime::goal::worktree::{
    is_git_worktree, materialize_goal_worktrees, GoalWorktreeMaterializeRequest,
};

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
    if !is_git_worktree(project_dir).await? {
        return Ok(());
    }

    // Skip worktree materialization if the repo has uncommitted changes.
    // This keeps `omk goal run --until-ready` usable on real projects
    // that naturally have untracked files.
    let clean = match tokio::time::timeout(
        std::time::Duration::from_secs(30),
        tokio::process::Command::new("git")
            .arg("-C")
            .arg(project_dir)
            .args(["status", "--porcelain"])
            .output(),
    )
    .await
    {
        Ok(Ok(out)) => {
            out.status.success() && String::from_utf8_lossy(&out.stdout).trim().is_empty()
        }
        _ => false,
    };
    if !clean {
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
