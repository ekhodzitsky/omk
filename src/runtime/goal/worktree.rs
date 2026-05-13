use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

const BRANCH_PREFIX: &str = "omk/goal";
const WORKTREE_PREFIX: &str = "goal";
const COMPONENT_MAX_CHARS: usize = 48;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoalWorktreePlan {
    pub goal_id: String,
    pub task_id: String,
    pub goal_component: String,
    pub task_component: String,
    pub branch_name: String,
    pub worktree_name: String,
    pub worktree_path: PathBuf,
}

pub fn plan_goal_worktree(
    worktrees_root: impl AsRef<Path>,
    goal_id: &str,
    task_id: &str,
) -> Result<GoalWorktreePlan> {
    let goal_component = normalize_identifier_component("goal id", goal_id)?;
    let task_component = normalize_identifier_component("task id", task_id)?;
    let fingerprint = stable_goal_task_fingerprint(goal_id, task_id);
    let branch_name = format!("{BRANCH_PREFIX}/{goal_component}/{task_component}-{fingerprint}");
    ensure_safe_branch_name(&branch_name).context("generated goal worktree branch is unsafe")?;
    let worktree_name =
        format!("{WORKTREE_PREFIX}-{goal_component}-{task_component}-{fingerprint}");
    ensure_safe_path_component("worktree name", &worktree_name)?;
    let worktree_path = worktrees_root.as_ref().join(&worktree_name);

    Ok(GoalWorktreePlan {
        goal_id: goal_id.to_string(),
        task_id: task_id.to_string(),
        goal_component,
        task_component,
        branch_name,
        worktree_name,
        worktree_path,
    })
}

pub fn plan_goal_worktrees<I, S>(
    worktrees_root: impl AsRef<Path>,
    goal_id: &str,
    task_ids: I,
) -> Result<Vec<GoalWorktreePlan>>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut branches = HashSet::new();
    let mut worktree_names = HashSet::new();
    let mut plans = Vec::new();

    for task_id in task_ids {
        let plan = plan_goal_worktree(worktrees_root.as_ref(), goal_id, task_id.as_ref())?;
        if !branches.insert(plan.branch_name.clone()) {
            anyhow::bail!("worktree plan collision for branch {}", plan.branch_name);
        }
        if !worktree_names.insert(plan.worktree_name.clone()) {
            anyhow::bail!(
                "worktree plan collision for worktree {}",
                plan.worktree_name
            );
        }
        plans.push(plan);
    }

    Ok(plans)
}

fn normalize_identifier_component(label: &str, value: &str) -> Result<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        anyhow::bail!("{label} cannot be empty");
    }
    if trimmed.chars().any(char::is_control) {
        anyhow::bail!("{label} contains control characters");
    }

    let mut normalized = String::new();
    let mut last_was_dash = false;
    for ch in trimmed.chars() {
        if ch.is_ascii_alphanumeric() {
            normalized.push(ch.to_ascii_lowercase());
            last_was_dash = false;
        } else if !last_was_dash {
            normalized.push('-');
            last_was_dash = true;
        }
    }

    let normalized = normalized.trim_matches('-');
    if normalized.is_empty() {
        anyhow::bail!("{label} has no safe path or branch characters");
    }

    Ok(truncate_component(normalized))
}

fn truncate_component(component: &str) -> String {
    let truncated: String = component.chars().take(COMPONENT_MAX_CHARS).collect();
    truncated.trim_matches('-').to_string()
}

fn stable_goal_task_fingerprint(goal_id: &str, task_id: &str) -> String {
    const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

    let mut hash = FNV_OFFSET;
    for byte in b"omk-goal-worktree" {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    for byte in goal_id
        .as_bytes()
        .iter()
        .chain([0xff].iter())
        .chain(task_id.as_bytes())
    {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    format!("{hash:016x}")
}

fn ensure_safe_path_component(label: &str, component: &str) -> Result<()> {
    if component.is_empty() || component == "." || component == ".." {
        anyhow::bail!("{label} is not a safe path component");
    }
    if component.contains('/') || component.contains('\\') {
        anyhow::bail!("{label} must not contain path separators");
    }
    if component.starts_with('.') || component.ends_with('.') {
        anyhow::bail!("{label} must not start or end with a dot");
    }
    if component.chars().any(char::is_control) {
        anyhow::bail!("{label} contains control characters");
    }
    Ok(())
}

fn ensure_safe_branch_name(branch: &str) -> Result<()> {
    if branch.is_empty() || branch == "@" {
        anyhow::bail!("branch name cannot be empty or @");
    }
    if branch.starts_with('/') || branch.ends_with('/') {
        anyhow::bail!("branch name cannot start or end with /");
    }
    if branch.contains("//") || branch.contains("..") || branch.contains("@{") {
        anyhow::bail!("branch name contains forbidden sequences");
    }
    if branch.ends_with('.') {
        anyhow::bail!("branch name cannot end with dot");
    }
    if branch
        .chars()
        .any(|ch| ch.is_control() || matches!(ch, ' ' | '~' | '^' | ':' | '?' | '*' | '[' | '\\'))
    {
        anyhow::bail!("branch name contains forbidden characters");
    }
    for component in branch.split('/') {
        if component.starts_with('.') || component.ends_with(".lock") {
            anyhow::bail!("branch name contains forbidden component {component}");
        }
    }
    Ok(())
}
