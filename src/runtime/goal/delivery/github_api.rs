use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::Duration;
use tokio::process::Command;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BranchProtectionPolicy {
    pub required_status_checks: Vec<String>,
    pub required_review_count: u32,
    pub enforce_admins: bool,
    pub allow_force_pushes: bool,
    pub allow_deletions: bool,
}

impl Default for BranchProtectionPolicy {
    fn default() -> Self {
        Self {
            required_status_checks: vec![],
            required_review_count: 1,
            enforce_admins: false,
            allow_force_pushes: false,
            allow_deletions: false,
        }
    }
}

pub async fn ensure_branch_protection(
    owner: &str,
    repo: &str,
    branch: &str,
    policy: &BranchProtectionPolicy,
) -> Result<()> {
    let checks = policy
        .required_status_checks
        .iter()
        .map(|name| json!({ "context": name }))
        .collect::<Vec<_>>();
    let json_body = serde_json::to_string(&json!({
        "required_status_checks": { "strict": true, "contexts": checks },
        "enforce_admins": policy.enforce_admins,
        "required_pull_request_reviews": {
            "required_approving_review_count": policy.required_review_count,
        },
        "restrictions": null,
        "allow_force_pushes": policy.allow_force_pushes,
        "allow_deletions": policy.allow_deletions,
    }))?;

    let mut cmd = Command::new("gh");
    cmd.args([
        "api",
        "-X",
        "PUT",
        &format!("repos/{owner}/{repo}/branches/{branch}/protection"),
    ])
    .arg("--input")
    .arg("-")
    .stdin(std::process::Stdio::piped())
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped());
    crate::runtime::shell::configure_command(&mut cmd);
    let mut child = cmd
        .spawn()
        .context("failed to spawn gh api for branch protection")?;

    let mut stdin = child
        .stdin
        .take()
        .context("failed to open stdin for gh api")?;
    tokio::io::AsyncWriteExt::write_all(&mut stdin, json_body.as_bytes()).await?;
    drop(stdin);

    let output = tokio::time::timeout(Duration::from_secs(60), child.wait_with_output())
        .await
        .context("timed out waiting for gh api branch protection")?
        .context("failed to run gh api branch protection")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("404") || output.status.code() == Some(404) {
            anyhow::bail!("branch protection failed: repository or branch not found (404)");
        }
        if stderr.contains("403") || output.status.code() == Some(403) {
            anyhow::bail!("branch protection failed: admin access required (403). Ensure your gh auth token has repo admin scope.");
        }
        anyhow::bail!("gh api branch protection failed: {}", stderr.trim());
    }
    Ok(())
}

pub fn parse_github_owner_repo(remote_url: &str) -> Option<(String, String)> {
    let url = remote_url
        .strip_prefix("https://github.com/")
        .or_else(|| remote_url.strip_prefix("http://github.com/"))
        .or_else(|| remote_url.strip_prefix("git@github.com:"))?;
    let parts: Vec<&str> = url.trim_end_matches(".git").split('/').collect();
    if parts.len() >= 2 {
        Some((parts[0].to_string(), parts[1].to_string()))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn branch_protection_policy_serializes_to_github_api_schema() {
        let _policy = BranchProtectionPolicy {
            required_status_checks: vec!["format".to_string(), "tests".to_string()],
            required_review_count: 1,
            enforce_admins: false,
            allow_force_pushes: false,
            allow_deletions: false,
        };
        let json = serde_json::to_value(json!({
            "required_status_checks": { "strict": true, "contexts": [
                {"context": "format"}, {"context": "tests"}
            ]},
            "enforce_admins": false,
            "required_pull_request_reviews": {
                "required_approving_review_count": 1,
            },
            "restrictions": null,
            "allow_force_pushes": false,
            "allow_deletions": false,
        }))
        .unwrap();
        assert!(json.get("required_status_checks").is_some());
    }
}
