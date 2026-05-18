use anyhow::Result;
use std::path::{Path, PathBuf};

use crate::runtime::gates::{
    detect_changed_files, gates_passed, load_or_detect_gates, run_gates_with_evidence,
};
use crate::runtime::goal::review::slop::{
    scan_for_slop, slop_confidence_from_findings, SlopFinding,
};
use crate::runtime::goal::state::GoalState;
use crate::runtime::goal::task_graph::{GoalDeliverySlice, GoalTaskGraph};
use crate::runtime::goal::verifier::scan_goal_security_findings;

/// Confidence threshold above which anti-slop issues are considered actionable.
pub const ANTI_SLOP_ACTIONABLE_THRESHOLD: f64 = 0.5;

/// A single review pass artifact for a slice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SliceReviewArtifact {
    pub kind: String,
    pub passed: bool,
    pub feedback: String,
    pub severity: String,
}

/// Outcome of a per-slice review.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SliceReviewOutcome {
    pub passed: bool,
    pub review_path: Option<PathBuf>,
    pub security_review_path: Option<PathBuf>,
    pub feedback: Option<String>,
    pub artifacts: Vec<SliceReviewArtifact>,
    pub slop_findings: Vec<SlopFinding>,
}

/// Compute anti-slop confidence from slice review artifacts and real slop findings.
/// Returns a value in [0.0, 1.0] where higher means more likely slop.
pub fn anti_slop_confidence(artifacts: &[SliceReviewArtifact]) -> f64 {
    let mut confidence: f64 = 0.0;
    for artifact in artifacts {
        if artifact.kind == "anti-slop" {
            continue;
        }
        if !artifact.passed {
            confidence += match artifact.kind.as_str() {
                "architect" => 0.20,
                "code" => 0.20,
                "test" => 0.20,
                "performance" => 0.15,
                "security" => 0.25,
                _ => 0.0,
            };
        }
    }
    confidence.min(1.0_f64)
}

/// Compute anti-slop confidence from both artifacts and real slop findings.
pub fn anti_slop_confidence_with_findings(
    artifacts: &[SliceReviewArtifact],
    findings: &[SlopFinding],
) -> f64 {
    let artifact_confidence = anti_slop_confidence(artifacts);
    let slop_confidence = slop_confidence_from_findings(findings);
    (artifact_confidence + slop_confidence).min(1.0)
}

/// Run review gates and security scan in the slice worktree and produce
/// pass/fail + human-readable feedback.
pub async fn review_slice(
    _slice: &GoalDeliverySlice,
    _goal_state: &GoalState,
    _task_graph: &GoalTaskGraph,
    worktree_path: &Path,
) -> Result<SliceReviewOutcome> {
    let gate_config = load_or_detect_gates(worktree_path).await;
    let gates = run_gates_with_evidence(&gate_config, worktree_path, None).await;
    let changed_files = detect_changed_files(worktree_path).await;
    let security_findings = scan_goal_security_findings(worktree_path, &changed_files).await?;

    let gates_ok = !gates.is_empty() && gates_passed(&gates);
    let security_ok = security_findings.is_empty();

    let passed = gates_ok && security_ok;

    let feedback = if passed {
        None
    } else {
        let mut parts = Vec::new();
        if !gates_ok {
            let failed = gates
                .iter()
                .filter(|g| !g.passed)
                .map(|g| g.name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            parts.push(format!("Gates failed: {failed}"));
        }
        if !security_ok {
            parts.push(format!(
                "Security findings ({}): {}",
                security_findings.len(),
                security_findings.join("; ")
            ));
        }
        Some(parts.join(". "))
    };

    let performance_ok = gates
        .iter()
        .filter(|gate| is_performance_gate(&gate.name))
        .any(|gate| gate.passed);

    // Run real anti-slop heuristics on changed files.
    let slop_findings = scan_for_slop(worktree_path, &changed_files);
    let _slop_confidence = slop_confidence_from_findings(&slop_findings);
    let anti_slop_passed = gates_ok && !changed_files.is_empty() && slop_findings.is_empty();
    let anti_slop_feedback = if anti_slop_passed {
        "Anti-slop review passed: changed-file evidence, passing gates, and no rough edges found"
            .to_string()
    } else {
        let mut parts = Vec::new();
        if !gates_ok {
            parts.push("missing changed-file evidence or failing gates".to_string());
        }
        if !slop_findings.is_empty() {
            let summary = slop_findings
                .iter()
                .map(|f| format!("{} at {:?}: {}", f.kind, f.line, f.message))
                .collect::<Vec<_>>()
                .join("; ");
            parts.push(format!("rough edges detected: {summary}"));
        }
        format!("Anti-slop review blocked: {}", parts.join(", "))
    };

    let artifacts = vec![
        SliceReviewArtifact {
            kind: "architect".to_string(),
            passed: gates_ok && !changed_files.is_empty(),
            feedback: if gates_ok && !changed_files.is_empty() {
                "Architecture review passed: changed-file evidence exists and gates passed"
                    .to_string()
            } else if changed_files.is_empty() {
                "Architecture review blocked: no changed-file evidence".to_string()
            } else {
                "Architecture review blocked: gates failed".to_string()
            },
            severity: if gates_ok && !changed_files.is_empty() {
                "low".to_string()
            } else {
                "high".to_string()
            },
        },
        SliceReviewArtifact {
            kind: "code".to_string(),
            passed: !changed_files.is_empty() && slop_findings.is_empty(),
            feedback: if changed_files.is_empty() {
                "Code review blocked: no changed files to inspect".to_string()
            } else if !slop_findings.is_empty() {
                format!(
                    "Code review blocked: {} rough edge(s) in changed files",
                    slop_findings.len()
                )
            } else {
                format!(
                    "Code review passed: {} changed file(s)",
                    changed_files.len()
                )
            },
            severity: if changed_files.is_empty() || !slop_findings.is_empty() {
                "high".to_string()
            } else {
                "low".to_string()
            },
        },
        SliceReviewArtifact {
            kind: "test".to_string(),
            passed: gates_ok,
            feedback: if gates_ok {
                "Test review passed: all required verification gates passed".to_string()
            } else {
                "Test review blocked: required verification gates failed".to_string()
            },
            severity: if gates_ok {
                "low".to_string()
            } else {
                "high".to_string()
            },
        },
        SliceReviewArtifact {
            kind: "security".to_string(),
            passed: security_ok,
            feedback: if security_ok {
                "Security review passed: no high-confidence secret markers found".to_string()
            } else {
                format!(
                    "Security review blocked: {} finding(s)",
                    security_findings.len()
                )
            },
            severity: if security_ok {
                "low".to_string()
            } else {
                "critical".to_string()
            },
        },
        SliceReviewArtifact {
            kind: "performance".to_string(),
            passed: performance_ok,
            feedback: if performance_ok {
                "Performance review passed: performance/benchmark gate passed".to_string()
            } else {
                "Performance review blocked: no performance or benchmark gate evidence".to_string()
            },
            severity: if performance_ok {
                "low".to_string()
            } else {
                "medium".to_string()
            },
        },
        SliceReviewArtifact {
            kind: "anti-slop".to_string(),
            passed: anti_slop_passed,
            feedback: anti_slop_feedback,
            severity: if anti_slop_passed {
                "low".to_string()
            } else {
                "medium".to_string()
            },
        },
    ];

    Ok(SliceReviewOutcome {
        passed,
        review_path: None,
        security_review_path: None,
        feedback,
        artifacts,
        slop_findings,
    })
}

fn is_performance_gate(name: &str) -> bool {
    let normalized = name.to_ascii_lowercase();
    normalized.contains("perf") || normalized.contains("bench")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slice_review_outcome_passed_no_feedback() {
        let outcome = SliceReviewOutcome {
            passed: true,
            review_path: None,
            security_review_path: None,
            feedback: None,
            artifacts: Vec::new(),
            slop_findings: Vec::new(),
        };
        assert!(outcome.passed);
        assert!(outcome.feedback.is_none());
    }

    #[test]
    fn slice_review_outcome_failed_has_feedback() {
        let outcome = SliceReviewOutcome {
            passed: false,
            review_path: None,
            security_review_path: None,
            feedback: Some("Gates failed: test".to_string()),
            artifacts: Vec::new(),
            slop_findings: Vec::new(),
        };
        assert!(!outcome.passed);
        assert_eq!(outcome.feedback, Some("Gates failed: test".to_string()));
    }

    #[test]
    fn anti_slop_confidence_all_passed_is_zero() {
        let artifacts = vec![
            SliceReviewArtifact {
                kind: "architect".to_string(),
                passed: true,
                feedback: "ok".to_string(),
                severity: "low".to_string(),
            },
            SliceReviewArtifact {
                kind: "code".to_string(),
                passed: true,
                feedback: "ok".to_string(),
                severity: "low".to_string(),
            },
        ];
        assert_eq!(anti_slop_confidence(&artifacts), 0.0);
    }

    #[test]
    fn anti_slop_confidence_failed_reviews_increases() {
        let artifacts = vec![
            SliceReviewArtifact {
                kind: "architect".to_string(),
                passed: false,
                feedback: "bad".to_string(),
                severity: "high".to_string(),
            },
            SliceReviewArtifact {
                kind: "code".to_string(),
                passed: false,
                feedback: "bad".to_string(),
                severity: "high".to_string(),
            },
            SliceReviewArtifact {
                kind: "security".to_string(),
                passed: false,
                feedback: "bad".to_string(),
                severity: "high".to_string(),
            },
        ];
        let confidence = anti_slop_confidence(&artifacts);
        assert!(
            confidence > ANTI_SLOP_ACTIONABLE_THRESHOLD,
            "expected confidence {confidence} > threshold {ANTI_SLOP_ACTIONABLE_THRESHOLD}"
        );
    }

    #[test]
    fn anti_slop_confidence_ignores_anti_slop_itself() {
        let artifacts = vec![SliceReviewArtifact {
            kind: "anti-slop".to_string(),
            passed: false,
            feedback: "bad".to_string(),
            severity: "high".to_string(),
        }];
        assert_eq!(anti_slop_confidence(&artifacts), 0.0);
    }

    #[test]
    fn anti_slop_confidence_caps_at_one() {
        let artifacts = vec![
            SliceReviewArtifact {
                kind: "architect".to_string(),
                passed: false,
                feedback: "bad".to_string(),
                severity: "high".to_string(),
            },
            SliceReviewArtifact {
                kind: "code".to_string(),
                passed: false,
                feedback: "bad".to_string(),
                severity: "high".to_string(),
            },
            SliceReviewArtifact {
                kind: "test".to_string(),
                passed: false,
                feedback: "bad".to_string(),
                severity: "high".to_string(),
            },
            SliceReviewArtifact {
                kind: "security".to_string(),
                passed: false,
                feedback: "bad".to_string(),
                severity: "critical".to_string(),
            },
            SliceReviewArtifact {
                kind: "performance".to_string(),
                passed: false,
                feedback: "bad".to_string(),
                severity: "medium".to_string(),
            },
        ];
        assert_eq!(anti_slop_confidence(&artifacts), 1.0);
    }

    #[test]
    fn is_performance_gate_detects_perf_and_bench() {
        assert!(is_performance_gate("perf-check"));
        assert!(is_performance_gate("benchmark"));
        assert!(!is_performance_gate("unit-test"));
    }
}
