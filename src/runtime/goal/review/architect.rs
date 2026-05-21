use std::path::{Path, PathBuf};
use std::process::Command;

use crate::runtime::goal::review::pass::ReviewPass;
use crate::runtime::goal::review::slice::{
    SliceReviewArtifact, SliceReviewContext, SliceReviewOutcome,
};

/// Architecture review pass enforcing file-size budgets and cross-module
/// import boundaries.
pub struct ArchitectReviewPass {
    max_file_loc: usize,
    #[allow(dead_code)]
    max_module_dependencies: usize,
    forbidden_cross_module_imports: Vec<(String, String)>,
    worktree_path: PathBuf,
}

#[allow(dead_code)]
impl ArchitectReviewPass {
    pub fn new() -> Self {
        let worktree_path = match std::env::current_dir() {
            Ok(p) => p,
            Err(_) => PathBuf::from("."),
        };
        Self {
            max_file_loc: 400,
            max_module_dependencies: 15,
            forbidden_cross_module_imports: Vec::new(),
            worktree_path,
        }
    }

    pub fn with_max_file_loc(mut self, n: usize) -> Self {
        self.max_file_loc = n;
        self
    }

    #[allow(dead_code)]
    pub fn with_max_module_dependencies(mut self, n: usize) -> Self {
        self.max_module_dependencies = n;
        self
    }

    pub fn with_forbidden_cross_module_imports(mut self, pairs: Vec<(String, String)>) -> Self {
        self.forbidden_cross_module_imports = pairs;
        self
    }

    pub fn with_worktree_path(mut self, path: impl AsRef<Path>) -> Self {
        self.worktree_path = path.as_ref().to_path_buf();
        self
    }

    /// Detect changed files by running `git status --porcelain` in the
    /// worktree.  Any failure (missing git, not a repo, etc.) returns an
    /// empty list — a soft-fail so the pass never panics.
    fn detect_changed_files(&self) -> Vec<String> {
        let output = Command::new("git")
            .args(["status", "--porcelain", "--untracked-files=all"])
            .current_dir(&self.worktree_path)
            .output();

        match output {
            Ok(o) if o.status.success() => {
                let mut files: Vec<String> = String::from_utf8_lossy(&o.stdout)
                    .lines()
                    .filter_map(|line| {
                        if line.len() < 4 {
                            return None;
                        }
                        let path = line.get(3..)?.trim();
                        if path.is_empty() {
                            return None;
                        }
                        let path = path.split(" -> ").last().unwrap_or(path).trim_matches('"');
                        if path.is_empty() {
                            None
                        } else {
                            Some(path.to_string())
                        }
                    })
                    .collect();
                files.sort();
                files.dedup();
                files
            }
            _ => Vec::new(),
        }
    }

    /// Check a single Rust source file against the configured rules.
    fn check_file(
        &self,
        file_name: &str,
        content: &str,
        forbidden: &[(String, String)],
    ) -> Vec<String> {
        let mut findings = Vec::new();
        let lines: Vec<&str> = content.lines().collect();

        // File size budget.
        if lines.len() > self.max_file_loc {
            findings.push(format!(
                "File {} has {} lines, exceeding architect budget of {}",
                file_name,
                lines.len(),
                self.max_file_loc
            ));
        }

        // Forbidden cross-module imports.
        for (from_mod, to_mod) in forbidden {
            if !file_name.starts_with(from_mod) {
                continue;
            }
            for line in &lines {
                let trimmed = line.trim();
                if !trimmed.starts_with("use crate::")
                    && !trimmed.starts_with("pub use crate::")
                    && !trimmed.starts_with("pub(crate) use crate::")
                {
                    continue;
                }
                if let Some(idx) = trimmed.find("crate::") {
                    let after = &trimmed[idx + "crate::".len()..];
                    let end = after.find([';', '{', '*', ',']).unwrap_or(after.len());
                    let import_path = after[..end].trim();
                    // Strip optional `as` alias.
                    let import_path = import_path.split_whitespace().next().unwrap_or(import_path);
                    let dir_path = format!("src/{}", import_path.replace("::", "/"));
                    let dir_norm = dir_path.trim_end_matches('/');
                    let to_norm = to_mod.trim_end_matches('/');
                    if dir_norm == to_norm || dir_norm.starts_with(&format!("{}/", to_norm)) {
                        findings.push(format!(
                            "Forbidden import in {}: `{}` crosses into `{}`",
                            file_name, trimmed, to_mod
                        ));
                    }
                }
            }
        }

        findings
    }
}

impl Default for ArchitectReviewPass {
    fn default() -> Self {
        Self::new()
    }
}

impl ReviewPass for ArchitectReviewPass {
    fn name(&self) -> &'static str {
        "architect"
    }

    fn run(&self, _ctx: &SliceReviewContext) -> SliceReviewOutcome {
        let changed_files = self.detect_changed_files();
        let mut all_findings: Vec<String> = Vec::new();

        let normalized_forbidden: Vec<(String, String)> = self
            .forbidden_cross_module_imports
            .iter()
            .map(|(a, b)| {
                let a_norm = if a.ends_with('/') {
                    a.clone()
                } else {
                    format!("{}/", a)
                };
                let b_norm = b.trim_end_matches('/').to_string();
                (a_norm, b_norm)
            })
            .collect();

        for file_name in &changed_files {
            if !file_name.ends_with(".rs") {
                continue;
            }
            let path = self.worktree_path.join(file_name);
            match std::fs::read_to_string(&path) {
                Ok(content) => {
                    all_findings.extend(self.check_file(
                        file_name,
                        &content,
                        &normalized_forbidden,
                    ));
                }
                Err(e) => {
                    all_findings.push(format!("Failed to read {}: {}", file_name, e));
                }
            }
        }

        let passed = all_findings.is_empty();

        let feedback = if passed {
            if changed_files.is_empty() {
                "Architecture review passed: no changed files to inspect".to_string()
            } else {
                format!(
                    "Architecture review passed: {} changed file(s) within budget",
                    changed_files.len()
                )
            }
        } else {
            format!("Architecture review blocked: {}", all_findings.join("; "))
        };

        let severity = if passed { "low" } else { "high" };

        SliceReviewOutcome {
            passed,
            review_path: None,
            security_review_path: None,
            feedback: if passed { None } else { Some(feedback.clone()) },
            artifacts: vec![SliceReviewArtifact {
                kind: "architect".to_string(),
                passed,
                feedback,
                severity: severity.to_string(),
            }],
            slop_findings: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn architect_pass_name_is_stable() {
        let pass = ArchitectReviewPass::new();
        assert_eq!("architect", pass.name());
    }

    #[test]
    fn architect_passes_when_changed_files_empty() {
        let tmp = tempfile::tempdir().unwrap();
        Command::new("git")
            .args(["init"])
            .current_dir(tmp.path())
            .output()
            .expect("git must be available for tests");

        let pass = ArchitectReviewPass::new().with_worktree_path(tmp.path());
        let outcome = pass.run(&SliceReviewContext);
        assert!(outcome.passed);
        let artifact = outcome
            .artifacts
            .iter()
            .find(|a| a.kind == "architect")
            .expect("architect artifact present");
        assert!(artifact.passed);
    }

    #[test]
    fn architect_passes_when_files_under_loc_limit() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("small.rs"), "fn main() {}\n").unwrap();
        Command::new("git")
            .args(["init"])
            .current_dir(tmp.path())
            .output()
            .expect("git must be available for tests");

        let pass = ArchitectReviewPass::new()
            .with_max_file_loc(10)
            .with_worktree_path(tmp.path());
        let outcome = pass.run(&SliceReviewContext);
        assert!(outcome.passed);
        let artifact = outcome
            .artifacts
            .iter()
            .find(|a| a.kind == "architect")
            .expect("architect artifact present");
        assert!(artifact.passed);
    }

    #[test]
    fn architect_fails_when_file_exceeds_loc_limit() {
        let tmp = tempfile::tempdir().unwrap();
        let mut content = String::new();
        for i in 0..15 {
            content.push_str(&format!("line {i}\n"));
        }
        std::fs::write(tmp.path().join("big.rs"), content).unwrap();
        Command::new("git")
            .args(["init"])
            .current_dir(tmp.path())
            .output()
            .expect("git must be available for tests");

        let pass = ArchitectReviewPass::new()
            .with_max_file_loc(10)
            .with_worktree_path(tmp.path());
        let outcome = pass.run(&SliceReviewContext);
        assert!(!outcome.passed);
        let artifact = outcome
            .artifacts
            .iter()
            .find(|a| a.kind == "architect")
            .expect("architect artifact present");
        assert!(!artifact.passed);
        assert!(
            artifact.feedback.contains("exceeding architect budget"),
            "expected file-size finding, got: {}",
            artifact.feedback
        );
    }

    #[test]
    fn architect_fails_on_forbidden_cross_module_import() {
        let tmp = tempfile::tempdir().unwrap();
        let cli_dir = tmp.path().join("src/cli");
        std::fs::create_dir_all(&cli_dir).unwrap();
        std::fs::write(
            cli_dir.join("main.rs"),
            "use crate::runtime::goal::state::GoalState;\nfn main() {}\n",
        )
        .unwrap();
        Command::new("git")
            .args(["init"])
            .current_dir(tmp.path())
            .output()
            .expect("git must be available for tests");

        let pass = ArchitectReviewPass::new()
            .with_worktree_path(tmp.path())
            .with_forbidden_cross_module_imports(vec![(
                "src/cli/".to_string(),
                "src/runtime/goal/".to_string(),
            )]);
        let outcome = pass.run(&SliceReviewContext);
        assert!(!outcome.passed);
        let artifact = outcome
            .artifacts
            .iter()
            .find(|a| a.kind == "architect")
            .expect("architect artifact present");
        assert!(!artifact.passed);
        assert!(
            artifact.feedback.contains("Forbidden import"),
            "expected forbidden-import finding, got: {}",
            artifact.feedback
        );
    }
}
