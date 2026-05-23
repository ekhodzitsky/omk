use crate::runtime::goal::review::slice::SliceReviewOutcome;

/// Aggregated review verdict for a slice. Used by auto-merge to decide
/// whether to proceed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AggregateReviewVerdict {
    pub all_passed: bool,
    /// Pass/fail per artifact kind. Keys are stable strings:
    /// "architect", "code", "test", "security",
    /// "performance", "anti-slop".
    pub per_pass: std::collections::BTreeMap<String, bool>,
    /// Human-readable summary listing which passes failed.
    pub blocking_reason: Option<String>,
}

/// Required passes for auto-merge. ALL must pass.
/// Set conservatively: same 6 as review_slice produces.
pub const REQUIRED_REVIEW_PASSES: &[&str] = &[
    "architect",
    "code",
    "test",
    "security",
    "performance",
    "anti-slop",
];

/// Compute aggregate verdict from SliceReviewOutcome.
///
/// Rules:
/// - all_passed = true iff for EVERY name in REQUIRED_REVIEW_PASSES,
///   there is an artifact with that kind AND artifact.passed == true.
/// - If any required pass is missing from artifacts → all_passed = false
///   (treat missing as fail; never auto-merge what we can't verify).
/// - blocking_reason lists failed/missing passes, distinguishing between
///   "failed" and "missing artifact".
pub fn aggregate_verdict(outcome: &SliceReviewOutcome) -> AggregateReviewVerdict {
    let mut per_pass = std::collections::BTreeMap::new();
    let mut failed = Vec::new();
    let mut missing = Vec::new();

    for kind in REQUIRED_REVIEW_PASSES {
        match outcome.artifacts.iter().find(|a| a.kind.as_str() == *kind) {
            Some(artifact) if artifact.passed => {
                per_pass.insert((*kind).to_string(), true);
            }
            Some(_) => {
                per_pass.insert((*kind).to_string(), false);
                failed.push(*kind);
            }
            None => {
                per_pass.insert((*kind).to_string(), false);
                missing.push(*kind);
            }
        }
    }

    let all_passed = per_pass.values().all(|&p| p);

    let blocking_reason = if all_passed {
        None
    } else {
        let mut parts = Vec::new();
        if !failed.is_empty() {
            parts.push(format!("failed: {}", failed.join(", ")));
        }
        if !missing.is_empty() {
            parts.push(format!("missing artifact: {}", missing.join(", ")));
        }
        Some(format!("review gate: {}", parts.join("; ")))
    };

    AggregateReviewVerdict {
        all_passed,
        per_pass,
        blocking_reason,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::goal::review::slice::{SliceReviewArtifact, SliceReviewOutcome};

    fn artifact(kind: &str, passed: bool) -> SliceReviewArtifact {
        SliceReviewArtifact {
            kind: kind.to_string(),
            passed,
            feedback: "feedback".to_string(),
            severity: "low".to_string(),
        }
    }

    #[test]
    fn aggregate_all_passed() {
        let outcome = SliceReviewOutcome {
            passed: true,
            review_path: None,
            security_review_path: None,
            feedback: None,
            artifacts: vec![
                artifact("architect", true),
                artifact("code", true),
                artifact("test", true),
                artifact("security", true),
                artifact("performance", true),
                artifact("anti-slop", true),
            ],
            slop_findings: Vec::new(),
        };
        let verdict = aggregate_verdict(&outcome);
        assert!(verdict.all_passed);
        assert!(verdict.blocking_reason.is_none());
    }

    #[test]
    fn aggregate_one_failed() {
        let outcome = SliceReviewOutcome {
            passed: false,
            review_path: None,
            security_review_path: None,
            feedback: None,
            artifacts: vec![
                artifact("architect", true),
                artifact("code", true),
                artifact("test", true),
                artifact("security", true),
                artifact("performance", true),
                artifact("anti-slop", false),
            ],
            slop_findings: Vec::new(),
        };
        let verdict = aggregate_verdict(&outcome);
        assert!(!verdict.all_passed);
        let reason = verdict.blocking_reason.unwrap();
        assert!(reason.contains("anti-slop"), "reason: {}", reason);
        assert!(reason.contains("failed"), "reason: {}", reason);
    }

    #[test]
    fn aggregate_missing_artifact() {
        let outcome = SliceReviewOutcome {
            passed: false,
            review_path: None,
            security_review_path: None,
            feedback: None,
            artifacts: vec![
                artifact("architect", true),
                artifact("code", true),
                artifact("test", true),
                artifact("performance", true),
                artifact("anti-slop", true),
            ],
            slop_findings: Vec::new(),
        };
        let verdict = aggregate_verdict(&outcome);
        assert!(!verdict.all_passed);
        let reason = verdict.blocking_reason.unwrap();
        assert!(reason.contains("security"), "reason: {}", reason);
        assert!(reason.contains("missing artifact"), "reason: {}", reason);
    }

    #[test]
    fn aggregate_ignores_unknown_kinds() {
        let outcome = SliceReviewOutcome {
            passed: true,
            review_path: None,
            security_review_path: None,
            feedback: None,
            artifacts: vec![
                artifact("architect", true),
                artifact("code", true),
                artifact("test", true),
                artifact("security", true),
                artifact("performance", true),
                artifact("anti-slop", true),
                artifact("unknown-kind", false),
            ],
            slop_findings: Vec::new(),
        };
        let verdict = aggregate_verdict(&outcome);
        assert!(verdict.all_passed);
    }
}
