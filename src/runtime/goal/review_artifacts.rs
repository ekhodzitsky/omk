use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use super::task_graph::{GoalTaskEvidence, GoalTaskGraph};
use crate::runtime::gates::GateResult;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GoalReviewPass {
    Architect,
    Code,
    Test,
    Security,
    Performance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GoalReviewArtifactStatus {
    Passed,
    Blocked,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalReviewArtifact {
    pub pass: GoalReviewPass,
    pub status: GoalReviewArtifactStatus,
    pub path: PathBuf,
    pub summary: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub known_gaps: Vec<String>,
}

const REQUIRED_REVIEW_PASSES: [GoalReviewPass; 5] = [
    GoalReviewPass::Architect,
    GoalReviewPass::Code,
    GoalReviewPass::Test,
    GoalReviewPass::Security,
    GoalReviewPass::Performance,
];

pub(crate) fn build_goal_review_artifacts(
    review_path: &Path,
    security_review_path: &Path,
    local_verify_done: bool,
    agent_execution_done: bool,
    gates: &[GateResult],
    changed_files: &[String],
    security_findings: &[String],
) -> Vec<GoalReviewArtifact> {
    let gates_ok = !gates.is_empty() && crate::runtime::gates::gates_passed(gates);
    let performance_gates: Vec<_> = gates
        .iter()
        .filter(|gate| is_performance_gate(&gate.name))
        .collect();
    let performance_ok =
        !performance_gates.is_empty() && performance_gates.iter().all(|gate| gate.passed);

    vec![
        artifact(
            GoalReviewPass::Architect,
            local_verify_done && agent_execution_done,
            review_path,
            "architecture evidence links planning, task graph, local verification, and execution",
            "architecture review is blocked until local verification and agent execution evidence exist",
        ),
        artifact(
            GoalReviewPass::Code,
            agent_execution_done && !changed_files.is_empty(),
            review_path,
            "code review has changed-file evidence to inspect",
            "code review is blocked until changed-file evidence exists",
        ),
        artifact(
            GoalReviewPass::Test,
            gates_ok,
            review_path,
            "test review passed because required verification gates passed",
            "test review is blocked until required verification gates pass",
        ),
        security_artifact(security_review_path, agent_execution_done, security_findings),
        artifact(
            GoalReviewPass::Performance,
            performance_ok,
            review_path,
            "performance review passed because a performance/benchmark gate passed",
            "performance review is blocked until performance or benchmark gate evidence exists",
        ),
    ]
}

pub(crate) fn review_artifacts_markdown(artifacts: &[GoalReviewArtifact]) -> String {
    let mut rows = ["| Pass | Status | Summary |", "| --- | --- | --- |"].join("\n");
    for artifact in artifacts {
        rows.push_str(&format!(
            "\n| {} | {} | {} |",
            artifact.pass.as_str(),
            artifact.status.as_str(),
            markdown_cell(&artifact.summary)
        ));
    }
    rows
}

pub(crate) fn review_task_evidence(
    evidence: &super::evidence::GoalReviewEvidence,
) -> Vec<GoalTaskEvidence> {
    evidence
        .review_artifacts
        .iter()
        .filter(|artifact| artifact.pass != GoalReviewPass::Security)
        .map(GoalReviewArtifact::to_task_evidence)
        .collect()
}

pub(crate) fn security_review_task_evidence(
    evidence: &super::evidence::GoalReviewEvidence,
) -> Vec<GoalTaskEvidence> {
    evidence
        .review_artifacts
        .iter()
        .filter(|artifact| artifact.pass == GoalReviewPass::Security)
        .map(GoalReviewArtifact::to_task_evidence)
        .collect()
}

pub(crate) fn collect_goal_review_artifacts(task_graph: &GoalTaskGraph) -> Vec<GoalReviewArtifact> {
    task_graph
        .tasks
        .iter()
        .flat_map(|task| task.evidence.iter())
        .filter_map(GoalReviewArtifact::from_task_evidence)
        .collect()
}

pub(crate) fn missing_review_artifact_gaps(artifacts: &[GoalReviewArtifact]) -> Vec<String> {
    REQUIRED_REVIEW_PASSES
        .iter()
        .filter(|pass| !artifacts.iter().any(|artifact| artifact.pass == **pass))
        .map(|pass| format!("{} review artifact is missing", pass.as_str()))
        .collect()
}

impl GoalReviewPass {
    fn as_str(self) -> &'static str {
        match self {
            GoalReviewPass::Architect => "architect",
            GoalReviewPass::Code => "code",
            GoalReviewPass::Test => "test",
            GoalReviewPass::Security => "security",
            GoalReviewPass::Performance => "performance",
        }
    }

    fn evidence_kind(self) -> &'static str {
        match self {
            GoalReviewPass::Architect => "architect_review",
            GoalReviewPass::Code => "code_review",
            GoalReviewPass::Test => "test_review",
            GoalReviewPass::Security => "security_review",
            GoalReviewPass::Performance => "performance_review",
        }
    }

    fn from_evidence_kind(kind: &str) -> Option<Self> {
        Some(match kind {
            "architect_review" => GoalReviewPass::Architect,
            "code_review" => GoalReviewPass::Code,
            "test_review" => GoalReviewPass::Test,
            "security_review" => GoalReviewPass::Security,
            "performance_review" => GoalReviewPass::Performance,
            _ => return None,
        })
    }
}

impl GoalReviewArtifactStatus {
    fn as_str(self) -> &'static str {
        match self {
            GoalReviewArtifactStatus::Passed => "passed",
            GoalReviewArtifactStatus::Blocked => "blocked",
        }
    }
}

impl GoalReviewArtifact {
    fn to_task_evidence(&self) -> GoalTaskEvidence {
        GoalTaskEvidence {
            kind: self.pass.evidence_kind().to_string(),
            path: self.path.clone(),
            summary: format!("{}: {}", self.status.as_str(), self.summary),
        }
    }

    fn from_task_evidence(evidence: &GoalTaskEvidence) -> Option<Self> {
        let pass = GoalReviewPass::from_evidence_kind(&evidence.kind)?;
        let (status, summary) = evidence
            .summary
            .split_once(": ")
            .map(|(status, summary)| (status, summary.to_string()))
            .unwrap_or((evidence.summary.as_str(), evidence.summary.clone()));
        let status = if status == "passed" {
            GoalReviewArtifactStatus::Passed
        } else {
            GoalReviewArtifactStatus::Blocked
        };
        Some(Self {
            pass,
            status,
            path: evidence.path.clone(),
            summary,
            known_gaps: Vec::new(),
        })
    }
}

fn artifact(
    pass: GoalReviewPass,
    passed: bool,
    path: &Path,
    passed_summary: &str,
    blocked_gap: &str,
) -> GoalReviewArtifact {
    GoalReviewArtifact {
        pass,
        status: if passed {
            GoalReviewArtifactStatus::Passed
        } else {
            GoalReviewArtifactStatus::Blocked
        },
        path: path.to_path_buf(),
        summary: if passed { passed_summary } else { blocked_gap }.to_string(),
        known_gaps: if passed {
            Vec::new()
        } else {
            vec![blocked_gap.to_string()]
        },
    }
}

fn security_artifact(
    path: &Path,
    agent_execution_done: bool,
    findings: &[String],
) -> GoalReviewArtifact {
    if !agent_execution_done {
        return artifact(
            GoalReviewPass::Security,
            false,
            path,
            "",
            "security review is blocked until agent execution evidence exists",
        );
    }
    if findings.is_empty() {
        return artifact(
            GoalReviewPass::Security,
            true,
            path,
            "security review passed because no high-confidence secret markers were found",
            "",
        );
    }
    GoalReviewArtifact {
        pass: GoalReviewPass::Security,
        status: GoalReviewArtifactStatus::Blocked,
        path: path.to_path_buf(),
        summary: format!(
            "security review is blocked by {} high-confidence secret marker(s)",
            findings.len()
        ),
        known_gaps: findings.to_vec(),
    }
}

fn is_performance_gate(name: &str) -> bool {
    let normalized = name.to_ascii_lowercase();
    normalized.contains("perf") || normalized.contains("bench")
}

fn markdown_cell(value: &str) -> String {
    value.replace('|', "\\|")
}
