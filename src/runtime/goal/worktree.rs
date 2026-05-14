use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Output;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;

mod conflict;

pub use conflict::{
    detect_goal_merge_conflicts, GoalMergeConflictCheckRequest, GoalMergeConflictEvidence,
};

const BRANCH_PREFIX: &str = "omk/goal";
const WORKTREE_PREFIX: &str = "goal";
const COMPONENT_MAX_CHARS: usize = 48;
const GIT_COMMAND_TIMEOUT: Duration = Duration::from_secs(30);

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

/// Request to turn deterministic goal worktree plans into real git worktrees.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoalWorktreeMaterializeRequest {
    pub repo_dir: PathBuf,
    pub worktrees_root: PathBuf,
    pub goal_dir: Option<PathBuf>,
    pub goal_id: String,
    pub task_ids: Vec<String>,
    pub dry_run: bool,
}

/// Result of a goal worktree materialization or dry-run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoalWorktreeMaterializeOutcome {
    pub dry_run: bool,
    pub plans: Vec<GoalWorktreePlan>,
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

/// Safely create planned goal worktrees after git cleanliness and collision checks.
pub async fn materialize_goal_worktrees(
    request: GoalWorktreeMaterializeRequest,
) -> Result<GoalWorktreeMaterializeOutcome> {
    let plans = plan_goal_worktrees(
        &request.worktrees_root,
        &request.goal_id,
        request.task_ids.iter().map(String::as_str),
    )?;

    ensure_git_worktree(&request.repo_dir).await?;
    ensure_clean_git_worktree(&request.repo_dir).await?;
    ensure_materialization_targets_are_available(&request.repo_dir, &plans).await?;
    if let Some(goal_dir) = &request.goal_dir {
        super::task_graph::ensure_worktree_delivery_targets(goal_dir, &plans).await?;
    }

    if request.dry_run {
        return Ok(GoalWorktreeMaterializeOutcome {
            dry_run: true,
            plans,
        });
    }

    tokio::fs::create_dir_all(&request.worktrees_root)
        .await
        .with_context(|| {
            format!(
                "Failed to create goal worktrees root: {}",
                request.worktrees_root.display()
            )
        })?;

    for plan in &plans {
        create_git_worktree(&request.repo_dir, plan).await?;
        if let Some(goal_dir) = &request.goal_dir {
            super::task_graph::record_worktree_delivery_metadata(goal_dir, plan).await?;
        }
    }

    Ok(GoalWorktreeMaterializeOutcome {
        dry_run: false,
        plans,
    })
}

async fn ensure_git_worktree(repo_dir: &Path) -> Result<()> {
    let output = git_output(
        repo_dir,
        git_args(&["rev-parse", "--is-inside-work-tree"]),
        "verify git repository",
    )
    .await?;
    if !output.status.success() || output_stdout(&output).trim() != "true" {
        anyhow::bail!(
            "goal worktree materialization requires a git repository: {}",
            repo_dir.display()
        );
    }
    Ok(())
}

async fn ensure_clean_git_worktree(repo_dir: &Path) -> Result<()> {
    let status = git_stdout(
        repo_dir,
        git_args(&["status", "--porcelain"]),
        "check git worktree status",
    )
    .await?;
    if status.trim().is_empty() {
        return Ok(());
    }

    let sample = status.lines().take(5).collect::<Vec<_>>().join("; ");
    anyhow::bail!(
        "goal worktree materialization requires a clean git worktree: {} has changes ({sample})",
        repo_dir.display()
    );
}

async fn ensure_materialization_targets_are_available(
    repo_dir: &Path,
    plans: &[GoalWorktreePlan],
) -> Result<()> {
    for plan in plans {
        if git_branch_exists(repo_dir, &plan.branch_name).await? {
            anyhow::bail!("goal worktree branch already exists: {}", plan.branch_name);
        }
        if path_is_occupied(&plan.worktree_path)? {
            anyhow::bail!(
                "goal worktree path already exists: {}",
                plan.worktree_path.display()
            );
        }
    }
    Ok(())
}

fn path_is_occupied(path: &Path) -> Result<bool> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => {
            Err(error).with_context(|| format!("Failed to inspect path: {}", path.display()))
        }
    }
}

async fn git_branch_exists(repo_dir: &Path, branch_name: &str) -> Result<bool> {
    let ref_name = format!("refs/heads/{branch_name}");
    let output = git_output(
        repo_dir,
        vec![
            OsString::from("show-ref"),
            OsString::from("--verify"),
            OsString::from("--quiet"),
            OsString::from(ref_name),
        ],
        "check branch availability",
    )
    .await?;

    match output.status.code() {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        _ => Err(git_failure("check branch availability", &output)),
    }
}

async fn create_git_worktree(repo_dir: &Path, plan: &GoalWorktreePlan) -> Result<()> {
    let output = git_output(
        repo_dir,
        vec![
            OsString::from("worktree"),
            OsString::from("add"),
            OsString::from("-b"),
            OsString::from(&plan.branch_name),
            plan.worktree_path.as_os_str().to_os_string(),
            OsString::from("HEAD"),
        ],
        "create goal worktree",
    )
    .await?;
    if output.status.success() {
        Ok(())
    } else {
        Err(git_failure("create goal worktree", &output))
    }
}

async fn git_stdout(repo_dir: &Path, args: Vec<OsString>, description: &str) -> Result<String> {
    let output = git_output(repo_dir, args, description).await?;
    if output.status.success() {
        Ok(output_stdout(&output))
    } else {
        Err(git_failure(description, &output))
    }
}

async fn git_output(repo_dir: &Path, args: Vec<OsString>, description: &str) -> Result<Output> {
    let mut command = Command::new("git");
    command.arg("-C").arg(repo_dir).args(args);
    timeout(GIT_COMMAND_TIMEOUT, command.output())
        .await
        .with_context(|| format!("Timed out while running git to {description}"))?
        .with_context(|| format!("Failed to run git to {description}"))
}

fn git_args(args: &[&str]) -> Vec<OsString> {
    args.iter().map(OsString::from).collect()
}

fn output_stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn git_failure(description: &str, output: &Output) -> anyhow::Error {
    anyhow::anyhow!(
        "git failed to {description}: status={} stdout={} stderr={}",
        output.status,
        String::from_utf8_lossy(&output.stdout).trim(),
        String::from_utf8_lossy(&output.stderr).trim()
    )
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
