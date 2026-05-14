#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GoalOracleAssessment {
    pub(crate) testable: bool,
    pub(crate) human_decisions_required: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GoalKind {
    Greenfield,
    Rewrite,
    Migration,
    Refactor,
    Audit,
    Bugfix,
    Performance,
    Docs,
    Mixed,
}

impl GoalKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Greenfield => "greenfield",
            Self::Rewrite => "rewrite",
            Self::Migration => "migration",
            Self::Refactor => "refactor",
            Self::Audit => "audit",
            Self::Bugfix => "bugfix",
            Self::Performance => "performance",
            Self::Docs => "docs",
            Self::Mixed => "mixed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GoalOracleEvidence {
    pub(crate) kind: GoalKind,
    pub(crate) passed: bool,
    pub(crate) checks: Vec<GoalOracleCheck>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GoalOracleCheck {
    pub(crate) name: String,
    pub(crate) passed: bool,
    pub(crate) gate: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GoalOracleGate {
    pub(crate) name: String,
    pub(crate) passed: bool,
}

#[allow(dead_code)]
#[path = "oracle/rewrite.rs"]
pub(crate) mod rewrite;
#[allow(dead_code)]
#[path = "oracle/surface.rs"]
pub(crate) mod surface;

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

pub(crate) fn classify_goal_kind(goal: &str) -> GoalKind {
    let lower = super::state::normalize_goal(goal).to_ascii_lowercase();
    let mut matches = Vec::new();
    if contains_any(&lower, &["rewrite", "port"]) {
        matches.push(GoalKind::Rewrite);
    }
    if contains_any(&lower, &["migrate", "migration"]) {
        matches.push(GoalKind::Migration);
    }
    if lower.contains("refactor") {
        matches.push(GoalKind::Refactor);
    }
    if lower.contains("audit") {
        matches.push(GoalKind::Audit);
    }
    if contains_any(&lower, &["bugfix", "bug fix", "fix "]) {
        matches.push(GoalKind::Bugfix);
    }
    if contains_any(&lower, &["performance", "benchmark", "perf"]) {
        matches.push(GoalKind::Performance);
    }
    if contains_any(&lower, &["docs", "documentation", "readme"]) {
        matches.push(GoalKind::Docs);
    }
    if lower.contains("greenfield")
        || (matches.is_empty() && contains_any(&lower, &["build", "create", "implement", "add "]))
    {
        matches.push(GoalKind::Greenfield);
    }

    matches.dedup();
    match matches.as_slice() {
        [] => GoalKind::Greenfield,
        [kind] => *kind,
        _ => GoalKind::Mixed,
    }
}

pub(crate) fn assess_goal_oracle_evidence(
    goal: &str,
    gates: &[GoalOracleGate],
) -> GoalOracleEvidence {
    let kind = classify_goal_kind(goal);
    let required = oracle_required_checks(kind);
    let checks = required
        .iter()
        .map(|name| oracle_check(name, gates))
        .collect::<Vec<_>>();
    GoalOracleEvidence {
        kind,
        passed: !checks.is_empty() && checks.iter().all(|check| check.passed),
        checks,
    }
}

pub(crate) fn oracle_evidence_json(evidence: &GoalOracleEvidence) -> serde_json::Value {
    serde_json::json!({
        "kind": evidence.kind.as_str(),
        "status": if evidence.passed { "passed" } else { "blocked" },
        "checks": evidence.checks.iter().map(|check| {
            serde_json::json!({
                "name": check.name,
                "status": if check.passed { "passed" } else { "blocked" },
                "gate": check.gate,
            })
        }).collect::<Vec<_>>(),
    })
}

fn oracle_required_checks(kind: GoalKind) -> &'static [&'static str] {
    match kind {
        GoalKind::Greenfield => &["acceptance", "smoke", "demo"],
        GoalKind::Rewrite | GoalKind::Migration | GoalKind::Refactor => {
            &["compatibility", "golden"]
        }
        GoalKind::Audit => &["audit"],
        GoalKind::Bugfix => &["regression"],
        GoalKind::Performance => &["performance"],
        GoalKind::Docs => &["docs"],
        GoalKind::Mixed => &["acceptance", "compatibility"],
    }
}

fn oracle_check(name: &str, gates: &[GoalOracleGate]) -> GoalOracleCheck {
    let needle = name.to_ascii_lowercase();
    let gate = gates.iter().find(|gate| {
        gate.passed
            && gate
                .name
                .to_ascii_lowercase()
                .split(|ch: char| !ch.is_ascii_alphanumeric())
                .any(|part| part == needle)
    });
    GoalOracleCheck {
        name: name.to_string(),
        passed: gate.is_some(),
        gate: gate.map(|gate| gate.name.clone()),
    }
}

fn contains_any(value: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| value.contains(needle))
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
