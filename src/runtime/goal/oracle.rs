#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GoalOracleAssessment {
    pub(crate) testable: bool,
    pub(crate) human_decisions_required: Vec<String>,
}

// Fixture-spike shape only; runtime rewrite execution belongs in a later slice.
#[allow(dead_code)]
pub(crate) mod rewrite {
    use std::collections::{BTreeMap, BTreeSet};

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct RewriteOracleObservation {
        pub(crate) stdout: String,
        pub(crate) stderr: String,
        pub(crate) exit_code: i32,
        pub(crate) file_artifacts: Vec<(String, String)>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct RewriteOracleComparison {
        pub(crate) compatible: bool,
        pub(crate) mismatches: Vec<RewriteOracleMismatch>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct RewriteOracleMismatch {
        pub(crate) field: RewriteOracleField,
        pub(crate) expected: String,
        pub(crate) actual: String,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) enum RewriteOracleField {
        Stdout,
        Stderr,
        ExitCode,
        FileArtifact { path: String },
    }

    pub(crate) fn compare_rewrite_oracle(
        expected: &RewriteOracleObservation,
        actual: &RewriteOracleObservation,
    ) -> RewriteOracleComparison {
        let mut mismatches = Vec::new();

        compare_text_field(
            RewriteOracleField::Stdout,
            &expected.stdout,
            &actual.stdout,
            &mut mismatches,
        );
        compare_text_field(
            RewriteOracleField::Stderr,
            &expected.stderr,
            &actual.stderr,
            &mut mismatches,
        );

        if expected.exit_code != actual.exit_code {
            mismatches.push(RewriteOracleMismatch {
                field: RewriteOracleField::ExitCode,
                expected: expected.exit_code.to_string(),
                actual: actual.exit_code.to_string(),
            });
        }

        compare_file_artifacts(
            &expected.file_artifacts,
            &actual.file_artifacts,
            &mut mismatches,
        );

        RewriteOracleComparison {
            compatible: mismatches.is_empty(),
            mismatches,
        }
    }

    fn compare_text_field(
        field: RewriteOracleField,
        expected: &str,
        actual: &str,
        mismatches: &mut Vec<RewriteOracleMismatch>,
    ) {
        if expected != actual {
            mismatches.push(RewriteOracleMismatch {
                field,
                expected: expected.to_string(),
                actual: actual.to_string(),
            });
        }
    }

    fn compare_file_artifacts(
        expected: &[(String, String)],
        actual: &[(String, String)],
        mismatches: &mut Vec<RewriteOracleMismatch>,
    ) {
        let expected = artifact_map(expected);
        let actual = artifact_map(actual);
        let paths: BTreeSet<_> = expected.keys().chain(actual.keys()).copied().collect();

        for path in paths {
            match (expected.get(path), actual.get(path)) {
                (Some(expected), Some(actual)) if expected != actual => {
                    mismatches.push(artifact_mismatch(path, expected, actual));
                }
                (Some(expected), None) => {
                    mismatches.push(artifact_mismatch(path, expected, "<missing>"));
                }
                (None, Some(actual)) => {
                    mismatches.push(artifact_mismatch(path, "<missing>", actual));
                }
                _ => {}
            }
        }
    }

    fn artifact_map(artifacts: &[(String, String)]) -> BTreeMap<&str, &str> {
        artifacts
            .iter()
            .map(|(path, contents)| (path.as_str(), contents.as_str()))
            .collect()
    }

    fn artifact_mismatch(path: &str, expected: &str, actual: &str) -> RewriteOracleMismatch {
        RewriteOracleMismatch {
            field: RewriteOracleField::FileArtifact {
                path: path.to_string(),
            },
            expected: expected.to_string(),
            actual: actual.to_string(),
        }
    }
}

impl GoalOracleAssessment {
    fn testable() -> Self {
        Self {
            testable: true,
            human_decisions_required: Vec::new(),
        }
    }

    fn blocked(reason: impl Into<String>) -> Self {
        Self {
            testable: false,
            human_decisions_required: vec![reason.into()],
        }
    }
}

pub(crate) fn assess_goal_oracle(goal: &str) -> GoalOracleAssessment {
    let normalized = super::state::normalize_goal(goal);
    let lower = normalized.to_ascii_lowercase();
    let word_count = normalized.split_whitespace().count();

    if normalized.is_empty() {
        return GoalOracleAssessment::blocked(
            "Define a non-empty engineering goal with testable success criteria.",
        );
    }

    if word_count < 3 {
        return GoalOracleAssessment::blocked(
            "Define testable success criteria before autonomous goal execution.",
        );
    }

    let has_vague_improvement = vague_improvement_patterns()
        .iter()
        .any(|pattern| lower.contains(pattern));
    let has_testable_signal = testable_goal_signals()
        .iter()
        .any(|signal| lower.contains(signal));

    if has_vague_improvement && !has_testable_signal {
        return GoalOracleAssessment::blocked(
            "Define testable success criteria for the requested improvement before autonomous goal execution.",
        );
    }

    GoalOracleAssessment::testable()
}

fn vague_improvement_patterns() -> &'static [&'static str] {
    &[
        "make it awesome",
        "make this awesome",
        "make it better",
        "make this better",
        "make better",
        "improve it",
        "improve this",
        "do magic",
        "state of art",
        "state-of-art",
    ]
}

fn testable_goal_signals() -> &'static [&'static str] {
    &[
        "acceptance",
        "api",
        "audit",
        "benchmark",
        "build",
        "cli",
        "compile",
        "coverage",
        "fix",
        "gate",
        "harden",
        "implement",
        "migrate",
        "performance",
        "proof",
        "refactor",
        "rewrite",
        "security",
        "test",
        "verify",
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oracle_blocks_vague_improvement_without_testable_signal() {
        let assessment = assess_goal_oracle("Make it awesome");

        assert!(!assessment.testable);
        assert!(assessment.human_decisions_required[0].contains("testable success criteria"));
    }

    #[test]
    fn oracle_allows_goal_with_testable_signal() {
        let assessment = assess_goal_oracle("Fix this repository until tests and proof pass");

        assert!(assessment.testable);
        assert!(assessment.human_decisions_required.is_empty());
    }
}
