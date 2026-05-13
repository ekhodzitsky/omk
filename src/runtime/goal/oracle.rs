#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GoalOracleAssessment {
    pub(crate) testable: bool,
    pub(crate) human_decisions_required: Vec<String>,
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
