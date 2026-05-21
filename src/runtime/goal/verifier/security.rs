use anyhow::Result;
use regex::Regex;
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SecurityFindingKind {
    PrivateKey,
    SecretAssignment,
    #[allow(dead_code)]
    SymlinkEscape,
    OversizedFile,
}

impl SecurityFindingKind {
    pub(crate) fn is_quarantine_only(&self) -> bool {
        matches!(self, SecurityFindingKind::OversizedFile)
    }
}

impl std::fmt::Display for SecurityFindingKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            SecurityFindingKind::PrivateKey => "private_key",
            SecurityFindingKind::SecretAssignment => "secret_assignment",
            SecurityFindingKind::SymlinkEscape => "symlink_escape",
            SecurityFindingKind::OversizedFile => "oversized_file",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SecurityFinding {
    pub(crate) path: String,
    pub(crate) kind: SecurityFindingKind,
    pub(crate) line: Option<usize>,
    pub(crate) evidence_snippet: Option<String>,
}

pub(crate) async fn scan_goal_security_findings(
    project_dir: &Path,
    changed_files: &[String],
) -> Result<Vec<String>> {
    let findings = scan_goal_security_findings_structured(project_dir, changed_files).await?;
    Ok(findings
        .into_iter()
        .map(|f| {
            format!(
                "{}:{} contains a high-confidence secret marker",
                f.path,
                f.line.map(|l| l.to_string()).unwrap_or_default()
            )
        })
        .collect())
}

pub(crate) async fn scan_goal_security_findings_structured(
    project_dir: &Path,
    changed_files: &[String],
) -> Result<Vec<SecurityFinding>> {
    let private_key = Regex::new(r"-----BEGIN [A-Z ]*PRIVATE KEY-----")?;
    let secret_assignment =
        Regex::new(r#"(?i)\b(api[_-]?key|secret|token|password)\b\s*[:=]\s*["'][^"']{16,}["']"#)?;
    let mut findings = Vec::new();

    // Canonicalize the project root once. When the path cannot be
    // canonicalized (the scanner can be exercised against ephemeral or
    // synthetic paths in tests) the per-file canonicalize step below also
    // fails, so the containment check is enforced consistently.
    let canonical_project_dir = tokio::fs::canonicalize(project_dir).await.ok();

    for changed_file in changed_files {
        let Some(path) = safe_project_file_path(project_dir, changed_file) else {
            continue;
        };
        // Resolve symlinks against the canonical project root before
        // reading. A changed file that escapes the project tree — for
        // example via a symlink planted by an upstream merge — must not be
        // scanned, both to avoid information disclosure through the
        // security review artifact and to keep findings traceable to repo
        // paths the reviewer can audit.
        let resolved = match tokio::fs::canonicalize(&path).await {
            Ok(canon) => canon,
            Err(_) => continue,
        };
        if let Some(root) = &canonical_project_dir {
            if !resolved.starts_with(root) {
                continue;
            }
        }
        let Ok(metadata) = tokio::fs::metadata(&resolved).await else {
            continue;
        };
        if !metadata.is_file() {
            continue;
        }
        if metadata.len() > 512 * 1024 {
            findings.push(SecurityFinding {
                path: changed_file.to_string(),
                kind: SecurityFindingKind::OversizedFile,
                line: None,
                evidence_snippet: Some(format!("file size {} bytes exceeds 512 KiB", metadata.len())),
            });
            continue;
        }
        let Ok(content) = tokio::fs::read_to_string(&resolved).await else {
            continue;
        };
        for (line_index, line) in content.lines().enumerate() {
            if private_key.is_match(line) {
                findings.push(SecurityFinding {
                    path: changed_file.to_string(),
                    kind: SecurityFindingKind::PrivateKey,
                    line: Some(line_index + 1),
                    evidence_snippet: Some(line.to_string()),
                });
            } else if secret_assignment.is_match(line) {
                findings.push(SecurityFinding {
                    path: changed_file.to_string(),
                    kind: SecurityFindingKind::SecretAssignment,
                    line: Some(line_index + 1),
                    evidence_snippet: Some(line.to_string()),
                });
            }
        }
    }

    Ok(findings)
}

fn safe_project_file_path(project_dir: &Path, changed_file: &str) -> Option<PathBuf> {
    let path = Path::new(changed_file);
    if path.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return None;
    }
    Some(project_dir.join(path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn safe_project_file_path_rejects_absolute_paths() {
        let root = Path::new("/tmp/omk-verifier-tests-nonexistent");
        assert!(safe_project_file_path(root, "/etc/passwd").is_none());
        assert!(safe_project_file_path(root, "../escape.txt").is_none());
    }

    #[test]
    fn safe_project_file_path_joins_relative_paths() {
        let root = Path::new("/tmp/omk-verifier-tests-nonexistent");
        let path = safe_project_file_path(root, "src/lib.rs").unwrap();
        assert_eq!(path, root.join("src/lib.rs"));
    }

    #[tokio::test]
    async fn scan_finds_inline_secret_assignment() {
        let dir = tempdir().unwrap();
        let project = dir.path();
        tokio::fs::write(
            project.join("creds.txt"),
            "api_key = \"AAAAAAAAAAAAAAAA-leaked-token-1\"\n",
        )
        .await
        .unwrap();

        let findings = scan_goal_security_findings(project, &["creds.txt".to_string()])
            .await
            .unwrap();

        assert_eq!(findings.len(), 1);
        assert!(findings[0].starts_with("creds.txt:1"));
    }

    #[tokio::test]
    async fn scan_skips_paths_that_escape_project_root_via_symlink() {
        // Plant a real secret outside the project root, then symlink to it
        // from inside the project. The scanner must canonicalize first and
        // refuse to read content that lives outside the project tree —
        // otherwise it would silently report a finding for `internal.rs`
        // that actually came from a file the reviewer cannot reach.
        let outside_dir = tempdir().unwrap();
        let outside_secret = outside_dir.path().join("stolen.txt");
        tokio::fs::write(
            &outside_secret,
            "api_key = \"BBBBBBBBBBBBBBBB-stolen-token-2\"\n",
        )
        .await
        .unwrap();

        let project_dir = tempdir().unwrap();
        let project = project_dir.path();
        let inside_link = project.join("internal.rs");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&outside_secret, &inside_link).unwrap();
        #[cfg(not(unix))]
        {
            // Symlinks on non-Unix targets require elevated privileges; we
            // only need the Unix behaviour to be locked down here.
            let _ = &inside_link;
            return;
        }

        let findings = scan_goal_security_findings(project, &["internal.rs".to_string()])
            .await
            .unwrap();

        assert!(
            findings.is_empty(),
            "scanner must refuse symlinked files that escape the project root; got {findings:?}",
        );
    }

    #[tokio::test]
    async fn scan_follows_symlink_that_stays_inside_project_root() {
        // The defense is "must stay inside the project tree", not "no
        // symlinks at all". A symlink whose target remains under the
        // project root is benign and the scanner should still inspect it.
        let project_dir = tempdir().unwrap();
        let project = project_dir.path();
        let real = project.join("real.txt");
        tokio::fs::write(&real, "password = \"CCCCCCCCCCCCCCCC-internal-secret-3\"\n")
            .await
            .unwrap();
        let link = project.join("alias.txt");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&real, &link).unwrap();
        #[cfg(not(unix))]
        {
            let _ = &link;
            return;
        }

        let findings = scan_goal_security_findings(project, &["alias.txt".to_string()])
            .await
            .unwrap();

        assert_eq!(findings.len(), 1);
        assert!(findings[0].starts_with("alias.txt:1"));
    }
}
