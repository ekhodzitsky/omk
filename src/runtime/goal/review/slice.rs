use anyhow::Result;
use std::path::{Path, PathBuf};

use crate::runtime::gates::{detect_changed_files, gates_passed, load_or_detect_gates, run_gates_with_evidence};
use crate::runtime::goal::state::GoalState;
use crate::runtime::goal::task_graph::{GoalDeliverySlice, GoalTaskGraph};
use crate::runtime::goal::verifier::scan_goal_security_findings;

/// Outcome of a per-slice review.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SliceReviewOutcome {
    pub passed: bool,
    pub review_path: Option<PathBuf>,
    pub security_review_path: Option<PathBuf>,
    pub feedback: Option<String>,
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

    Ok(SliceReviewOutcome {
        passed,
        review_path: None,
        security_review_path: None,
        feedback,
    })
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
        };
        assert!(!outcome.passed);
        assert_eq!(outcome.feedback, Some("Gates failed: test".to_string()));
    }
}
