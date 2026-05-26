use serde::{Deserialize, Serialize};

use super::diagnosis::{DiagnosisReport, StagnationCause};

/// Risk level associated with a recovery plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
}

impl std::fmt::Display for RiskLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RiskLevel::Low => write!(f, "low"),
            RiskLevel::Medium => write!(f, "medium"),
            RiskLevel::High => write!(f, "high"),
        }
    }
}

/// Strategy for recovering from stagnation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecoveryStrategy {
    ReduceScope,
    MockExternalDeps,
    RefactorApproach,
    EscalateToHuman,
}

impl std::fmt::Display for RecoveryStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RecoveryStrategy::ReduceScope => write!(f, "reduce_scope"),
            RecoveryStrategy::MockExternalDeps => write!(f, "mock_external_deps"),
            RecoveryStrategy::RefactorApproach => write!(f, "refactor_approach"),
            RecoveryStrategy::EscalateToHuman => write!(f, "escalate_to_human"),
        }
    }
}

/// A single recovery task.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RecoveryTask {
    pub description: String,
    pub target_files: Vec<String>,
    pub expected_outcome: String,
}

/// A complete recovery plan.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RecoveryPlan {
    pub cause: StagnationCause,
    pub strategy: RecoveryStrategy,
    pub confidence: f64,
    pub description: String,
    pub suggested_tasks: Vec<RecoveryTask>,
    pub risk_level: RiskLevel,
    pub estimated_tokens: Option<u64>,
}

/// Generates recovery plans from diagnosis reports.
#[derive(Debug, Clone, Default)]
pub struct RecoveryPlanner;

impl RecoveryPlanner {
    pub fn new() -> Self {
        Self
    }

    /// Generate a recovery plan from a diagnosis report.
    pub fn plan(&self, diagnosis: &DiagnosisReport) -> RecoveryPlan {
        let (strategy, description, tasks, risk_level, estimated_tokens) =
            match diagnosis.cause {
                StagnationCause::TestFlakiness => (
                    RecoveryStrategy::MockExternalDeps,
                    format!(
                        "mock external dependencies causing flakiness in {}",
                        diagnosis.affected_gates.join(", ")
                    ),
                    vec![RecoveryTask {
                        description: "introduce stubs or mocks for flaky external dependency"
                            .to_string(),
                        target_files: diagnosis.affected_files.clone(),
                        expected_outcome: "gate results become stable across iterations".to_string(),
                    }],
                    RiskLevel::Low,
                    Some(5000),
                ),
                StagnationCause::ScopeTooLarge => (
                    RecoveryStrategy::ReduceScope,
                    "split the current goal into smaller subtasks with fewer files each".to_string(),
                    vec![RecoveryTask {
                        description: "decompose current task into subtasks touching <5 files each"
                            .to_string(),
                        target_files: diagnosis.affected_files.clone(),
                        expected_outcome: "each subtask achieves measurable proof_score progress"
                            .to_string(),
                    }],
                    RiskLevel::Medium,
                    Some(8000),
                ),
                StagnationCause::ExternalDependencyBroken => (
                    RecoveryStrategy::EscalateToHuman,
                    "external dependency is broken and cannot be auto-recovered".to_string(),
                    vec![RecoveryTask {
                        description: "operator must fix the external environment or dependency"
                            .to_string(),
                        target_files: Vec::new(),
                        expected_outcome: "gate passes after dependency is restored".to_string(),
                    }],
                    RiskLevel::High,
                    None,
                ),
                StagnationCause::CircularFix => (
                    RecoveryStrategy::RefactorApproach,
                    "circular modification detected; rollback to last checkpoint and try a new approach".to_string(),
                    vec![
                        RecoveryTask {
                            description: "rollback to last known good checkpoint".to_string(),
                            target_files: diagnosis.affected_files.clone(),
                            expected_outcome: "restore stable state before circular changes".to_string(),
                        },
                        RecoveryTask {
                            description: "apply a different implementation approach".to_string(),
                            target_files: diagnosis.affected_files.clone(),
                            expected_outcome: "avoid the same modification cycle".to_string(),
                        },
                    ],
                    RiskLevel::Medium,
                    Some(10000),
                ),
                StagnationCause::ReviewRejectionLoop => (
                    RecoveryStrategy::EscalateToHuman,
                    "review has been rejected multiple times with similar reasons".to_string(),
                    vec![RecoveryTask {
                        description: "operator review required to resolve repeated rejections"
                            .to_string(),
                        target_files: Vec::new(),
                        expected_outcome: "clear guidance on how to satisfy review criteria".to_string(),
                    }],
                    RiskLevel::High,
                    None,
                ),
                StagnationCause::InefficientExploration => (
                    RecoveryStrategy::ReduceScope,
                    "too many tokens spent for little progress; reduce scope".to_string(),
                    vec![RecoveryTask {
                        description: "narrow scope to the smallest change that improves proof_score"
                            .to_string(),
                        target_files: diagnosis.affected_files.clone(),
                        expected_outcome: "better token efficiency (lower tokens per 1% progress)"
                            .to_string(),
                    }],
                    RiskLevel::Low,
                    Some(6000),
                ),
                StagnationCause::Unknown => (
                    RecoveryStrategy::ReduceScope,
                    "stagnation detected but root cause is unclear; reduce scope as safe default"
                        .to_string(),
                    vec![RecoveryTask {
                        description: "reduce scope and re-evaluate progress".to_string(),
                        target_files: Vec::new(),
                        expected_outcome: "restore forward progress or clarify root cause".to_string(),
                    }],
                    RiskLevel::Low,
                    Some(5000),
                ),
            };

        RecoveryPlan {
            cause: diagnosis.cause,
            strategy,
            confidence: diagnosis.confidence,
            description,
            suggested_tasks: tasks,
            risk_level,
            estimated_tokens,
        }
    }
}
