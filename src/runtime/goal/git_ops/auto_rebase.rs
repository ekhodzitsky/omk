use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::git::GitRepo;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RebaseOutcome {
    Clean,
    ConflictUnresolvable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConflictClassification {
    Safe { reason: String },
    Unsafe { reason: String },
}

/// Attempt an auto-rebase of `branch` onto `base_branch`.
/// If trivial conflicts are detected they are resolved automatically.
/// Returns the rebase outcome together with an optional conflict classification.
pub async fn attempt_auto_rebase(
    repo_dir: &Path,
    branch: &str,
    base_branch: &str,
) -> Result<(RebaseOutcome, Option<ConflictClassification>)> {
    validate_git_ref(branch)?;
    validate_git_ref(base_branch)?;

    let repo =
        GitRepo::open(repo_dir).map_err(|e| anyhow::anyhow!("failed to open git repo: {e}"))?;

    repo.checkout(branch)
        .await
        .map_err(|e| anyhow::anyhow!("git checkout {branch} failed: {e}"))?;

    let fetch_ok = repo.fetch("origin").await.is_ok();
    let base_ref = if fetch_ok {
        format!("origin/{base_branch}")
    } else {
        base_branch.to_string()
    };

    if repo.rebase(&base_ref).await.is_err() {
        // Rebase failed — inspect conflicts before deciding to abort.
        let classification = classify_conflicts(repo_dir).await?;
        match classification {
            ConflictClassification::Safe { .. } => {
                let repo = GitRepo::open(repo_dir)
                    .map_err(|e| anyhow::anyhow!("failed to open git repo: {e}"))?;

                let conflict_files = repo
                    .conflicted_files()
                    .await
                    .map_err(|e| anyhow::anyhow!("failed to list conflicted files: {e}"))?;

                for file in &conflict_files {
                    resolve_safe_conflict(repo_dir, file)
                        .await
                        .with_context(|| format!("failed to resolve safe conflict in {file}"))?;
                    repo.add(file)
                        .await
                        .map_err(|e| anyhow::anyhow!("git add failed for {file}: {e}"))?;
                }

                // Continue the rebase.
                if repo.rebase_continue().await.is_err() {
                    let _ = repo.rebase_abort().await;
                    return Ok((
                        RebaseOutcome::ConflictUnresolvable,
                        Some(ConflictClassification::Unsafe {
                            reason: "rebase --continue failed after resolving safe conflicts"
                                .to_string(),
                        }),
                    ));
                }

                return Ok((RebaseOutcome::Clean, Some(classification)));
            }
            ConflictClassification::Unsafe { .. } => {
                let _ = repo.rebase_abort().await;
                return Ok((RebaseOutcome::ConflictUnresolvable, Some(classification)));
            }
        }
    }

    Ok((RebaseOutcome::Clean, None))
}

async fn classify_conflicts(repo_dir: &Path) -> Result<ConflictClassification> {
    let repo =
        GitRepo::open(repo_dir).map_err(|e| anyhow::anyhow!("failed to open git repo: {e}"))?;

    // Check for delete conflicts which are always unsafe.
    let porcelain = repo
        .status_porcelain()
        .await
        .map_err(|e| anyhow::anyhow!("git status failed: {e}"))?;

    for line in porcelain.lines() {
        if line.len() < 2 {
            continue;
        }
        let xy = &line[..2];
        // DD / DU / UD = deletion conflicts — always unsafe.
        // UU / AA / AU / UA = content conflicts — may be safe.
        if xy == "DD" || xy == "DU" || xy == "UD" {
            return Ok(ConflictClassification::Unsafe {
                reason: format!(
                    "delete conflict detected (status {xy}): requires manual resolution"
                ),
            });
        }
    }

    let conflict_files = repo
        .conflicted_files()
        .await
        .map_err(|e| anyhow::anyhow!("failed to list conflicted files: {e}"))?;

    if conflict_files.is_empty() {
        return Ok(ConflictClassification::Unsafe {
            reason: "rebase failed but no conflicted files detected".to_string(),
        });
    }

    for file in &conflict_files {
        let path = repo_dir.join(file);
        let content = match tokio::fs::read_to_string(&path).await {
            Ok(c) => c,
            Err(_) => {
                return Ok(ConflictClassification::Unsafe {
                    reason: format!("file '{file}' appears to be binary or unreadable"),
                });
            }
        };

        if !is_conflict_safe(&content) {
            return Ok(ConflictClassification::Unsafe {
                reason: format!("file '{file}' contains substantive conflicts"),
            });
        }
    }

    Ok(ConflictClassification::Safe {
        reason: format!(
            "all {} conflicted files contain only trivial conflicts (whitespace/line-ending/comments)",
            conflict_files.len()
        ),
    })
}

fn is_conflict_safe(content: &str) -> bool {
    let regions = extract_conflict_regions(content);
    if regions.is_empty() {
        // No conflict markers — could be binary or a special conflict type.
        return false;
    }

    for (ours, theirs) in regions {
        if !are_regions_trivially_different(&ours, &theirs) {
            return false;
        }
    }

    true
}

fn extract_conflict_regions(content: &str) -> Vec<(String, String)> {
    let mut regions = Vec::new();
    let lines: Vec<&str> = content.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        if lines[i].starts_with("<<<<<<< ") {
            i += 1;
            let mut ours = Vec::new();
            while i < lines.len() && !lines[i].starts_with("=======") {
                ours.push(lines[i]);
                i += 1;
            }
            i += 1; // skip =======
            let mut theirs = Vec::new();
            while i < lines.len() && !lines[i].starts_with(">>>>>>> ") {
                theirs.push(lines[i]);
                i += 1;
            }
            i += 1; // skip >>>>>>>
            regions.push((ours.join("\n"), theirs.join("\n")));
        } else {
            i += 1;
        }
    }

    regions
}

fn are_regions_trivially_different(ours: &str, theirs: &str) -> bool {
    let normalize = |s: &str| -> Vec<String> {
        s.lines()
            .map(|line| line.trim_end())
            .filter(|line| !line.is_empty())
            .map(|line| line.to_string())
            .collect()
    };

    let ours_norm = normalize(ours);
    let theirs_norm = normalize(theirs);

    if ours_norm == theirs_norm {
        return true;
    }

    // Check if both sides are comment-only.
    let is_comment_only = |lines: &[String]| -> bool {
        lines.iter().all(|line| {
            let trimmed = line.trim_start();
            trimmed.starts_with("//")
                || trimmed.starts_with("/*")
                || trimmed.starts_with("*/")
                || trimmed.starts_with('*')
                || trimmed.starts_with('#')
        })
    };

    is_comment_only(&ours_norm) && is_comment_only(&theirs_norm)
}

async fn resolve_safe_conflict(repo_dir: &Path, file: &str) -> Result<()> {
    let path = repo_dir.join(file);
    let content = tokio::fs::read_to_string(&path)
        .await
        .with_context(|| format!("failed to read conflicted file: {}", path.display()))?;

    let resolved = resolve_conflict_markers(&content);
    tokio::fs::write(&path, resolved)
        .await
        .with_context(|| format!("failed to write resolved file: {}", path.display()))?;
    Ok(())
}

fn resolve_conflict_markers(content: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let mut result = Vec::new();
    let mut i = 0;

    while i < lines.len() {
        if lines[i].starts_with("<<<<<<< ") {
            i += 1;
            let mut ours = Vec::new();
            while i < lines.len() && !lines[i].starts_with("=======") {
                ours.push(lines[i]);
                i += 1;
            }
            i += 1; // skip =======
            while i < lines.len() && !lines[i].starts_with(">>>>>>> ") {
                i += 1;
            }
            i += 1; // skip >>>>>>>

            // For safe conflicts both sides are semantically equivalent.
            // Pick the "ours" side with normalized trailing whitespace.
            for line in &ours {
                result.push(line.trim_end().to_string());
            }
        } else {
            result.push(lines[i].to_string());
            i += 1;
        }
    }

    result.join("\n")
}

fn validate_git_ref(name: &str) -> Result<()> {
    if name.starts_with('-') {
        anyhow::bail!("invalid git ref name: cannot start with '-': {name}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_conflict_regions_single_region() {
        let content = "line1\n<<<<<<< HEAD\nours\n=======\ntheirs\n>>>>>>> branch\nline2";
        let regions = extract_conflict_regions(content);
        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0].0, "ours");
        assert_eq!(regions[0].1, "theirs");
    }

    #[test]
    fn extract_conflict_regions_multiple_regions() {
        let content =
            "<<<<<<< HEAD\na\n=======\nb\n>>>>>>> b1\n<<<<<<< HEAD\nc\n=======\nd\n>>>>>>> b2";
        let regions = extract_conflict_regions(content);
        assert_eq!(regions.len(), 2);
        assert_eq!(regions[0].0, "a");
        assert_eq!(regions[0].1, "b");
        assert_eq!(regions[1].0, "c");
        assert_eq!(regions[1].1, "d");
    }

    #[test]
    fn are_regions_trivially_different_whitespace_only() {
        assert!(are_regions_trivially_different("hello \n", "hello\n"));
        assert!(are_regions_trivially_different("hello\r\n", "hello\n"));
        assert!(are_regions_trivially_different("hello  \n\n", "hello\n"));
    }

    #[test]
    fn are_regions_trivially_different_substantive_changes() {
        assert!(!are_regions_trivially_different("foo", "bar"));
        assert!(!are_regions_trivially_different("foo bar", "foobar"));
    }

    #[test]
    fn are_regions_trivially_different_comment_only() {
        assert!(are_regions_trivially_different("// old\n", "// new\n"));
        assert!(are_regions_trivially_different("# old\n", "# new\n"));
        assert!(!are_regions_trivially_different("// old\n", "code\n"));
    }

    #[test]
    fn resolve_conflict_markers_replaces_marker() {
        let content = "before\n<<<<<<< HEAD\nours\n=======\ntheirs\n>>>>>>> branch\nafter";
        let resolved = resolve_conflict_markers(content);
        assert_eq!(resolved, "before\nours\nafter");
    }

    #[test]
    fn is_conflict_safe_detects_safe_and_unsafe() {
        let safe = "<<<<<<< HEAD\nhello \n=======\nhello\n>>>>>>> branch";
        assert!(is_conflict_safe(safe));

        let unsafe_content = "<<<<<<< HEAD\nfoo\n=======\nbar\n>>>>>>> branch";
        assert!(!is_conflict_safe(unsafe_content));
    }

    #[test]
    fn is_conflict_safe_empty_regions_returns_false() {
        assert!(!is_conflict_safe("no markers here"));
    }
}
