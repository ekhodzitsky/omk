use serde_json::{json, Value};

use crate::runtime::gates::{gates_passed, GateResult};

use super::super::state::{
    GOAL_ARTIFACTS_DIR, GOAL_REVIEW_ARTIFACTS_DIR, GOAL_REVIEW_FILE, GOAL_SECURITY_REVIEW_FILE,
};

pub(crate) fn collect_review_artifacts(
    review_done: bool,
    security_review_done: bool,
    gates: &[GateResult],
    changed_files: &[String],
) -> Vec<Value> {
    if !review_done && !security_review_done {
        return Vec::new();
    }

    let gates_ok = !gates.is_empty() && gates_passed(gates);
    let performance_ok = gates
        .iter()
        .filter(|gate| is_performance_gate(&gate.name))
        .any(|gate| gate.passed);
    let review_path =
        format!("{GOAL_ARTIFACTS_DIR}/{GOAL_REVIEW_ARTIFACTS_DIR}/{GOAL_REVIEW_FILE}");
    let security_path =
        format!("{GOAL_ARTIFACTS_DIR}/{GOAL_REVIEW_ARTIFACTS_DIR}/{GOAL_SECURITY_REVIEW_FILE}");
    let gate_evidence = if gates.is_empty() {
        "no gate evidence recorded".to_string()
    } else {
        let gates = gates
            .iter()
            .map(|gate| {
                let status = if gate.passed { "passed" } else { "failed" };
                format!("{}={status}", gate.name)
            })
            .collect::<Vec<_>>()
            .join(", ");
        format!("gate results: {gates}")
    };
    let changed_file_evidence = if changed_files.is_empty() {
        "no changed-file evidence captured".to_string()
    } else {
        format!("changed files: {}", changed_files.join(", "))
    };
    let anti_slop_ok = review_done && gates_ok && !changed_files.is_empty();

    vec![
        review_artifact(ReviewArtifact {
            pass: "architect",
            passed: review_done,
            path: &review_path,
            summary: "architecture review artifact is present",
            evidence: vec![format!("controller review path: {review_path}")],
            risks: vec![
                "architecture fit is inferred from local controller artifacts only".to_string(),
            ],
            known_gaps: review_gaps(review_done, "architect review artifact is missing"),
            recommended_next_step: if review_done {
                "Carry architecture evidence into the PR for human review."
            } else {
                "Run `omk goal review latest` after agent execution evidence exists."
            },
        }),
        review_artifact(ReviewArtifact {
            pass: "code",
            passed: review_done && !changed_files.is_empty(),
            path: &review_path,
            summary: if changed_files.is_empty() {
                "code review is blocked until changed-file evidence exists"
            } else {
                "code review has changed-file evidence to inspect"
            },
            evidence: vec![changed_file_evidence.clone()],
            risks: vec![
                "code review is deterministic and does not replace maintainer judgment"
                    .to_string(),
            ],
            known_gaps: review_gaps(
                review_done && !changed_files.is_empty(),
                "code review is blocked until changed-file evidence exists",
            ),
            recommended_next_step: if changed_files.is_empty() {
                "Capture project mutation evidence before PR readiness review."
            } else {
                "Inspect the changed files in the PR diff."
            },
        }),
        review_artifact(ReviewArtifact {
            pass: "test",
            passed: gates_ok,
            path: &review_path,
            summary: if gates_ok {
                "test review passed because required verification gates passed"
            } else {
                "test review is blocked until required verification gates pass"
            },
            evidence: vec![gate_evidence.clone()],
            risks: vec!["gate coverage only reflects configured local gates".to_string()],
            known_gaps: review_gaps(
                gates_ok,
                "test review is blocked until required verification gates pass",
            ),
            recommended_next_step: if gates_ok {
                "Keep the gate output attached to the PR evidence."
            } else {
                "Run or fix required local verification gates."
            },
        }),
        review_artifact(ReviewArtifact {
            pass: "security",
            passed: security_review_done,
            path: &security_path,
            summary: "security review artifact is present",
            evidence: vec![format!("security review path: {security_path}")],
            risks: vec!["secret scanning is a local high-confidence heuristic".to_string()],
            known_gaps: review_gaps(security_review_done, "security review artifact is missing"),
            recommended_next_step: if security_review_done {
                "Review security evidence and changed files before merge."
            } else {
                "Run `omk goal review latest` and resolve any security findings."
            },
        }),
        review_artifact(ReviewArtifact {
            pass: "performance",
            passed: performance_ok,
            path: &review_path,
            summary: if performance_ok {
                "performance review passed because a performance/benchmark gate passed"
            } else {
                "performance review is blocked until performance or benchmark gate evidence exists"
            },
            evidence: vec![gate_evidence],
            risks: vec![
                "performance confidence depends on the configured perf/benchmark gate"
                    .to_string(),
            ],
            known_gaps: review_gaps(
                performance_ok,
                "performance review is blocked until performance or benchmark gate evidence exists",
            ),
            recommended_next_step: if performance_ok {
                "Keep the performance gate evidence with the PR."
            } else {
                "Add or run a performance/benchmark gate for this goal."
            },
        }),
        review_artifact(ReviewArtifact {
            pass: "anti-slop",
            passed: anti_slop_ok,
            path: &review_path,
            summary: if anti_slop_ok {
                "anti-slop review passed because changed-file evidence and local gates are present"
            } else {
                "anti-slop review is blocked until changed-file evidence and passing gates exist"
            },
            evidence: vec![changed_file_evidence],
            risks: vec![
                "anti-slop evidence is deterministic and cannot replace a human maintainability review"
                    .to_string(),
            ],
            known_gaps: review_gaps(
                anti_slop_ok,
                "anti-slop review is blocked until changed-file evidence and passing gates exist",
            ),
            recommended_next_step: if anti_slop_ok {
                "Keep the PR small and carry the simplification rationale into review."
            } else {
                "Run a focused cleanup review after local gates and changed-file evidence exist."
            },
        }),
    ]
}

pub(crate) fn review_artifact_known_gaps(artifacts: &[Value]) -> Vec<String> {
    artifacts
        .iter()
        .filter_map(|artifact| artifact.get("known_gaps").and_then(Value::as_array))
        .flat_map(|gaps| gaps.iter().filter_map(Value::as_str).map(str::to_string))
        .collect()
}

pub(crate) fn review_artifacts_passed(artifacts: &[Value]) -> bool {
    !artifacts.is_empty()
        && artifacts.iter().all(|artifact| {
            artifact
                .get("status")
                .and_then(Value::as_str)
                .is_some_and(|status| status == "passed")
        })
}

struct ReviewArtifact<'a> {
    pass: &'a str,
    passed: bool,
    path: &'a str,
    summary: &'a str,
    evidence: Vec<String>,
    risks: Vec<String>,
    known_gaps: Vec<String>,
    recommended_next_step: &'a str,
}

fn review_artifact(artifact: ReviewArtifact<'_>) -> Value {
    let status = if artifact.passed { "passed" } else { "blocked" };
    json!({
        "pass": artifact.pass,
        "status": status,
        "path": artifact.path,
        "summary": artifact.summary,
        "evidence": artifact.evidence,
        "risks": artifact.risks,
        "known_gaps": artifact.known_gaps,
        "recommended_next_step": artifact.recommended_next_step,
    })
}

fn review_gaps(passed: bool, blocked_gap: &str) -> Vec<String> {
    if passed {
        Vec::new()
    } else {
        vec![blocked_gap.to_string()]
    }
}

fn is_performance_gate(name: &str) -> bool {
    let normalized = name.to_ascii_lowercase();
    normalized.contains("perf") || normalized.contains("bench")
}
