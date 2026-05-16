use std::ffi::OsString;
use std::path::Path;

use anyhow::{Context, Result};

const GIT_COMMAND_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

pub(crate) async fn resolve_base_branch(repo_dir: &Path) -> Option<String> {
    for branch in ["main", "master"] {
        let output = git_command(
            repo_dir,
            vec![
                OsString::from("show-ref"),
                OsString::from("--verify"),
                OsString::from("--quiet"),
                OsString::from(format!("refs/heads/{branch}")),
            ],
        )
        .await
        .ok()?;
        if output.status.success() {
            return Some(branch.to_string());
        }
    }
    None
}

pub(crate) async fn create_integrator_branch(
    repo_dir: &Path,
    integrator_branch: &str,
    base_branch: &str,
) -> anyhow::Result<()> {
    let output = git_command(
        repo_dir,
        vec![
            OsString::from("checkout"),
            OsString::from("-b"),
            OsString::from(integrator_branch),
            OsString::from(base_branch),
        ],
    )
    .await?;
    if output.status.success() {
        Ok(())
    } else {
        anyhow::bail!("git checkout -b failed: {}", output_stderr(&output))
    }
}

pub(crate) async fn merge_tree_is_clean(
    repo_dir: &Path,
    branch: &str,
    integrator_branch: &str,
) -> anyhow::Result<()> {
    let output = git_command(
        repo_dir,
        vec![
            OsString::from("merge-tree"),
            OsString::from(integrator_branch),
            OsString::from(branch),
        ],
    )
    .await?;
    if !output.status.success() {
        anyhow::bail!("git merge-tree failed: {}", output_stderr(&output));
    }
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if stdout.contains("<<<<<<<") || stdout.contains("=======") || stdout.contains(">>>>>>>") {
        anyhow::bail!("merge-tree predicts conflicts between {integrator_branch} and {branch}");
    }
    Ok(())
}

pub(crate) async fn merge_branch_into_integrator(
    repo_dir: &Path,
    branch: &str,
    integrator_branch: &str,
) -> anyhow::Result<()> {
    let checkout = git_command(
        repo_dir,
        vec![
            OsString::from("checkout"),
            OsString::from(integrator_branch),
        ],
    )
    .await?;
    if !checkout.status.success() {
        anyhow::bail!(
            "git checkout integrator failed: {}",
            output_stderr(&checkout)
        );
    }

    let output = git_command(
        repo_dir,
        vec![
            OsString::from("merge"),
            OsString::from(branch),
            OsString::from("--no-edit"),
        ],
    )
    .await?;
    if output.status.success() {
        return Ok(());
    }

    let rebase = git_command(
        repo_dir,
        vec![
            OsString::from("checkout"),
            OsString::from(branch),
        ],
    )
    .await?;
    if !rebase.status.success() {
        anyhow::bail!(
            "git merge failed and rebase checkout failed: {}",
            output_stderr(&output)
        );
    }
    let rebase = git_command(
        repo_dir,
        vec![
            OsString::from("rebase"),
            OsString::from(integrator_branch),
        ],
    )
    .await?;
    if !rebase.status.success() {
        let _ = git_command(
            repo_dir,
            vec![
                OsString::from("rebase"),
                OsString::from("--abort"),
            ],
        )
        .await;
        let _ = git_command(
            repo_dir,
            vec![
                OsString::from("checkout"),
                OsString::from(integrator_branch),
            ],
        )
        .await;
        anyhow::bail!(
            "git merge failed and auto-rebase failed: {}",
            output_stderr(&output)
        );
    }

    let push = git_command(
        repo_dir,
        vec![
            OsString::from("push"),
            OsString::from("-f"),
            OsString::from("origin"),
            OsString::from(branch),
        ],
    )
    .await?;
    if !push.status.success() {
        anyhow::bail!(
            "git merge failed and rebase push failed: {}",
            output_stderr(&push)
        );
    }

    let checkout = git_command(
        repo_dir,
        vec![
            OsString::from("checkout"),
            OsString::from(integrator_branch),
        ],
    )
    .await?;
    if !checkout.status.success() {
        anyhow::bail!(
            "git checkout integrator failed after rebase: {}",
            output_stderr(&checkout)
        );
    }
    let output = git_command(
        repo_dir,
        vec![
            OsString::from("merge"),
            OsString::from(branch),
            OsString::from("--no-edit"),
        ],
    )
    .await?;
    if output.status.success() {
        Ok(())
    } else {
        anyhow::bail!(
            "git merge failed even after auto-rebase: {}",
            output_stderr(&output)
        );
    }
}

pub(crate) async fn push_branch(repo_dir: &Path, branch: &str) -> anyhow::Result<()> {
    let output = git_command(
        repo_dir,
        vec![
            OsString::from("push"),
            OsString::from("-u"),
            OsString::from("origin"),
            OsString::from(branch),
        ],
    )
    .await?;
    if output.status.success() {
        Ok(())
    } else {
        anyhow::bail!("git push failed: {}", output_stderr(&output))
    }
}

pub(super) async fn git_command(repo_dir: &Path, args: Vec<OsString>) -> Result<std::process::Output> {
    let mut command = tokio::process::Command::new("git");
    command.arg("-C").arg(repo_dir).args(args);
    tokio::time::timeout(GIT_COMMAND_TIMEOUT, command.output())
        .await
        .with_context(|| "Timed out while running git command")?
        .with_context(|| "Failed to run git command")
}

fn output_stderr(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stderr).trim().to_string()
}
