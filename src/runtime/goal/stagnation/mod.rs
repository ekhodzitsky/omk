pub mod checkpoint;
pub mod collector;
pub mod detector;
pub mod diagnosis;
pub mod recovery;

#[cfg(test)]
mod tests;

pub use checkpoint::{RecoveryCheckpoint, RecoveryCheckpointError};
pub use collector::{IterationMetrics, StagnationCollector, StagnationCollectorError};
pub use detector::{StagnationDetector, StagnationReport, StagnationThresholds};
pub use diagnosis::{DiagnosisEngine, DiagnosisReport, StagnationCause};
pub use recovery::{RecoveryPlan, RecoveryPlanner, RecoveryStrategy, RecoveryTask, RiskLevel};
