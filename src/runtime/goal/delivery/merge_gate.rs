/*! Merge pre-flight gate checks.

# Manual Test Procedure for Real GitHub Repos

Since the automated test suite mocks the `gh` CLI responses via the
`GoalGithubPrClient` trait, validating the merge gate against a real
GitHub repository requires the following manual procedure:

## Prerequisites

1. A GitHub repository with branch protection enabled on `main` or `master`.
2. The `gh` CLI installed and authenticated (`gh auth status`).
3. A goal that has been delivered with an integrator PR (`omk goal run ...`).

## Test Scenarios

### 1. Gated policy success path
```bash
# Create a goal with gated merge policy
omk goal run "Add a small README change" --merge-policy gated --until-ready

# Verify the PR is merged automatically after CI passes
gh pr view <pr-url> --json state  # should show MERGED
```

### 2. Manual policy success path
```bash
omk goal run "Add a small README change" --merge-policy manual --until-ready

# PR should be created but not merged
omk goal merge latest
```

### 3. Disabled policy
```bash
omk goal run "Add a small README change" --merge-policy disabled --until-ready

# omk goal merge latest should fail with:
# "Goal '...' has merge policy 'disabled' which does not permit merge"
```

### 4. Merge blocked on CI failure
```bash
# Introduce a failing check (e.g., break a test) in the goal branch
omk goal run "Break a test on purpose" --merge-policy gated --until-ready

# Should block with:
# "gated merge blocked: required CI check failed: ..."
```

### 5. Merge blocked on review
```bash
# Request changes on the integrator PR in GitHub UI
omk goal merge latest

# Should block with:
# "PR has requested changes from reviewers"
```

### 6. Merge blocked on conflicts
```bash
# Push a conflicting commit to main while the goal is running
# Then attempt merge
omk goal merge latest

# Should block with:
# "PR has merge conflicts"
```

### 7. Merge blocked on missing branch protection
```bash
# Temporarily remove branch protection from main
omk goal merge latest

# Should block with:
# "branch protection not configured for base branch 'main'"
```

## Expected Evidence

After a successful merge, the goal proof (`proof.json`) should contain:
- `status`: `"ready"`
- `readiness`: `"ready: PR merged after passing merge gate"`
- An artifact with `kind`: `"pr_merge"` and `kind`: `"delivery_evidence"`
*/
use anyhow::{Context, Result};
use serde::Deserialize;
use std::time::Duration;
use tokio::process::Command;

use super::parse_github_owner_repo;

const GH_TIMEOUT: Duration = Duration::from_secs(60);

/// Run the merge pre-flight gate checks for a PR URL.
///
/// Checks:
/// 1. CI checks passing via `gh pr checks`
/// 2. PR is mergeable via `gh pr view --json mergeable`
/// 3. Review decision is approved via `gh pr view --json reviewDecision`
/// 4. Branch protection is configured via `gh api`
pub async fn run_merge_pre_flight(pr_url: &str) -> Result<()> {
    check_pr_ci_status(pr_url).await?;
    check_pr_mergeable_and_review(pr_url).await?;
    check_branch_protection_for_pr(pr_url).await?;
    Ok(())
}

async fn check_pr_ci_status(pr_url: &str) -> Result<()> {
    let mut command = Command::new("gh");
    command.arg("pr").arg("checks").arg(pr_url);
    let output = tokio::time::timeout(GH_TIMEOUT, command.output())
        .await
        .with_context(|| format!("timed out waiting for gh pr checks {pr_url}"))?
        .with_context(|| format!("failed to start gh pr checks {pr_url}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("no checks reported") {
            anyhow::bail!("no CI checks reported for PR");
        }
        anyhow::bail!("gh pr checks failed: {}", stderr.trim());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<_> = stdout.lines().collect();
    if lines.is_empty() {
        anyhow::bail!("no CI checks found for PR");
    }

    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let parts: Vec<_> = trimmed.split_whitespace().collect();
        if parts.len() >= 2 {
            let state = parts[1];
            if state == "fail" {
                anyhow::bail!("CI check '{}' failed", parts[0]);
            }
            if state != "pass" {
                anyhow::bail!("CI check '{}' is not passing (state: {})", parts[0], state);
            }
        }
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
struct PrViewJson {
    mergeable: Option<bool>,
    #[serde(rename = "mergeStateStatus")]
    merge_state_status: Option<String>,
    #[serde(rename = "reviewDecision")]
    review_decision: Option<String>,
}

async fn check_pr_mergeable_and_review(pr_url: &str) -> Result<()> {
    let mut command = Command::new("gh");
    command
        .arg("pr")
        .arg("view")
        .arg(pr_url)
        .arg("--json")
        .arg("mergeable,mergeStateStatus,reviewDecision");
    let output = tokio::time::timeout(GH_TIMEOUT, command.output())
        .await
        .with_context(|| format!("timed out waiting for gh pr view {pr_url}"))?
        .with_context(|| format!("failed to start gh pr view {pr_url}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("gh pr view failed: {}", stderr.trim());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let info: PrViewJson = serde_json::from_str(&stdout)
        .with_context(|| format!("failed to parse gh pr view output for {pr_url}"))?;

    if info.mergeable == Some(false) {
        anyhow::bail!("PR has merge conflicts");
    }

    match info.merge_state_status.as_deref() {
        Some("DIRTY") => anyhow::bail!("PR has merge conflicts (mergeStateStatus: DIRTY)"),
        Some("BLOCKED") => {
            // Could be blocked by review or protection; fall through to review check
        }
        Some("UNSTABLE") => anyhow::bail!("PR checks are unstable or failing"),
        Some("BEHIND") => anyhow::bail!("PR branch is behind base branch"),
        _ => {}
    }

    match info.review_decision.as_deref() {
        Some("APPROVED") => {}
        Some("CHANGES_REQUESTED") => {
            anyhow::bail!("PR has requested changes from reviewers");
        }
        Some("REVIEW_REQUIRED") => anyhow::bail!("PR requires review approval"),
        _ => {
            // Unknown or missing review decision; allow if mergeable is true
        }
    }

    Ok(())
}

#[derive(Debug, Deserialize)]
struct BaseRef {
    #[serde(rename = "baseRefName")]
    base_ref_name: String,
}

async fn check_branch_protection_for_pr(pr_url: &str) -> Result<()> {
    let (owner, repo) = match parse_github_owner_repo(pr_url) {
        Some(pair) => pair,
        None => {
            // Not a GitHub PR URL; skip branch protection check
            return Ok(());
        }
    };

    // Fetch base branch name from PR
    let mut command = Command::new("gh");
    command
        .arg("pr")
        .arg("view")
        .arg(pr_url)
        .arg("--json")
        .arg("baseRefName");
    let output = tokio::time::timeout(GH_TIMEOUT, command.output())
        .await
        .with_context(|| format!("timed out waiting for gh pr view {pr_url}"))?
        .with_context(|| format!("failed to start gh pr view {pr_url}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("gh pr view failed: {}", stderr.trim());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let base_ref: BaseRef = serde_json::from_str(&stdout)
        .with_context(|| "failed to parse base branch from gh pr view output")?;

    let mut command = Command::new("gh");
    command.arg("api").arg(format!(
        "repos/{owner}/{repo}/branches/{}/protection",
        base_ref.base_ref_name
    ));
    let output = tokio::time::timeout(GH_TIMEOUT, command.output())
        .await
        .with_context(|| {
            format!(
                "timed out waiting for gh api branch protection {owner}/{repo}/{}",
                base_ref.base_ref_name
            )
        })?
        .with_context(|| {
            format!(
                "failed to start gh api branch protection {owner}/{repo}/{}",
                base_ref.base_ref_name
            )
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("404") || output.status.code() == Some(404) {
            anyhow::bail!(
                "branch protection not configured for base branch '{}'",
                base_ref.base_ref_name
            );
        }
        anyhow::bail!("gh api branch protection failed: {}", stderr.trim());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pr_view_json_parsing() {
        let json = r#"{"mergeable":true,"mergeStateStatus":"CLEAN","reviewDecision":"APPROVED"}"#;
        let parsed: PrViewJson = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.mergeable, Some(true));
        assert_eq!(parsed.merge_state_status.as_deref(), Some("CLEAN"));
        assert_eq!(parsed.review_decision.as_deref(), Some("APPROVED"));
    }

    #[test]
    fn base_ref_parsing() {
        let json = r#"{"baseRefName":"main"}"#;
        let parsed: BaseRef = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.base_ref_name, "main");
    }
}
