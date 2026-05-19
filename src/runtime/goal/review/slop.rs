use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// A single rough-edge finding that contributes to anti-slop confidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SlopFinding {
    pub kind: SlopKind,
    pub file: PathBuf,
    pub line: Option<usize>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SlopKind {
    FileTooLong,
    BannedPattern,
    TodoFixmeHack,
}

impl std::fmt::Display for SlopKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SlopKind::FileTooLong => write!(f, "file-too-long"),
            SlopKind::BannedPattern => write!(f, "banned-pattern"),
            SlopKind::TodoFixmeHack => write!(f, "todo-fixme-hack"),
        }
    }
}

/// Scan changed files for rough edges that indicate slop.
///
/// Heuristics:
/// - File size > 400 lines (AGENTS.md violation)
/// - `unwrap()`, `expect()`, `panic!()` in production code (AGENTS.md banned)
/// - `TODO`, `FIXME`, `HACK` comments in production code (AGENTS.md Tier 3)
pub(crate) fn scan_for_slop(worktree_path: &Path, changed_files: &[String]) -> Vec<SlopFinding> {
    let mut findings = Vec::new();
    let banned_patterns: &[(&str, &str)] = &[
        ("unwrap()", "unwrap() is banned in production code"),
        ("expect(", "expect() is banned in production code"),
        ("panic!(", "panic!() is banned in production code"),
    ];
    let todo_patterns: &[&str] = &["TODO", "FIXME", "HACK"];

    for file_name in changed_files {
        let path = worktree_path.join(file_name);
        if !path.is_file() {
            continue;
        }

        // Only scan source files.
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if !matches!(ext, "rs" | "js" | "ts" | "py" | "go" | "md") {
            continue;
        }

        let content = std::fs::read_to_string(&path).unwrap_or_default();
        let lines: Vec<&str> = content.lines().collect();

        // File size heuristic.
        if lines.len() > 400 {
            findings.push(SlopFinding {
                kind: SlopKind::FileTooLong,
                file: path.clone(),
                line: None,
                message: format!(
                    "file has {} lines, exceeding AGENTS.md 400-line limit",
                    lines.len()
                ),
            });
        }

        // Banned patterns and TODO/FIXME/HACK heuristics.
        for (line_no, line) in lines.iter().enumerate() {
            let line_number = line_no + 1;

            // Skip comment lines for banned patterns (they may be documenting the rule).
            let trimmed = line.trim();
            if trimmed.starts_with("//") || trimmed.starts_with("#") || trimmed.starts_with("*") {
                // But still scan for TODO/FIXME/HACK in comments.
                for pattern in todo_patterns {
                    if trimmed.contains(pattern) {
                        findings.push(SlopFinding {
                            kind: SlopKind::TodoFixmeHack,
                            file: path.clone(),
                            line: Some(line_number),
                            message: format!("found '{pattern}' in production code comment"),
                        });
                        break;
                    }
                }
                continue;
            }

            for (pattern, message) in banned_patterns {
                if line.contains(pattern) {
                    findings.push(SlopFinding {
                        kind: SlopKind::BannedPattern,
                        file: path.clone(),
                        line: Some(line_number),
                        message: message.to_string(),
                    });
                }
            }
        }
    }

    findings
}

/// Compute a normalized anti-slop confidence in [0.0, 1.0] from real findings.
pub(crate) fn slop_confidence_from_findings(findings: &[SlopFinding]) -> f64 {
    if findings.is_empty() {
        return 0.0;
    }
    let mut score = 0.0;
    let mut counted_files = HashSet::new();
    for finding in findings {
        match finding.kind {
            SlopKind::FileTooLong => score += 0.15,
            SlopKind::BannedPattern => score += 0.20,
            SlopKind::TodoFixmeHack => score += 0.10,
        }
        counted_files.insert(finding.file.clone());
    }
    // Cap per-file contributions to avoid a single file dominating.
    let file_count = counted_files.len().max(1);
    (score / file_count as f64).min(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_finds_file_too_long() {
        let tmp = tempfile::tempdir().unwrap();
        let mut content = String::new();
        for i in 0..410 {
            content.push_str(&format!("line {i}\n"));
        }
        std::fs::write(tmp.path().join("long.rs"), content).unwrap();

        let findings = scan_for_slop(tmp.path(), &["long.rs".to_string()]);
        assert!(
            findings
                .iter()
                .any(|f| matches!(f.kind, SlopKind::FileTooLong)),
            "expected file-too-long finding"
        );
    }

    #[test]
    fn scan_finds_banned_patterns() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("bad.rs"), "fn main() { x.unwrap(); }\n").unwrap();

        let findings = scan_for_slop(tmp.path(), &["bad.rs".to_string()]);
        assert!(
            findings
                .iter()
                .any(|f| matches!(f.kind, SlopKind::BannedPattern)),
            "expected banned pattern finding"
        );
    }

    #[test]
    fn scan_finds_todo_in_comment() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("todo.rs"),
            "// TODO fix this\nfn main() {}\n",
        )
        .unwrap();

        let findings = scan_for_slop(tmp.path(), &["todo.rs".to_string()]);
        assert!(
            findings
                .iter()
                .any(|f| matches!(f.kind, SlopKind::TodoFixmeHack)),
            "expected todo finding"
        );
    }

    #[test]
    fn confidence_empty_is_zero() {
        assert_eq!(slop_confidence_from_findings(&[]), 0.0);
    }

    #[test]
    fn confidence_capped_at_one() {
        let findings = vec![
            SlopFinding {
                kind: SlopKind::BannedPattern,
                file: PathBuf::from("a.rs"),
                line: Some(1),
                message: "unwrap".to_string(),
            };
            10
        ];
        assert_eq!(slop_confidence_from_findings(&findings), 1.0);
    }
}
