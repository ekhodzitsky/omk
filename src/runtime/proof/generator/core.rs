use std::path::Path;

use crate::runtime::events::{EventReader, RunId};
use crate::runtime::gates::GateResult;
use crate::runtime::proof::{
    ChangedFile, GateStatus, Proof, ProofGate, ProofStatus,
};

impl super::ProofGenerator {
    pub async fn from_events(run_id: &RunId, event_log: &Path) -> anyhow::Result<Proof> {
        let log_summary = EventReader::summary(event_log).await?;
        let events = EventReader::read_all(event_log).await?;
        let mut proof = Self::from_event_list(run_id, &events)?;
        if log_summary.parse_failures > 0 {
            proof.known_gaps.push(format!(
                "event log parse failures: {} malformed line(s) skipped",
                log_summary.parse_failures
            ));
        }
        Ok(proof)
    }

    /// Generate a proof from verification gate results directly (for modes that don't use events yet).
    #[allow(dead_code)]
    pub fn from_gate_results(
        run_id: RunId,
        gate_results: &[GateResult],
        changed_files: &[String],
        known_gaps: &[String],
    ) -> Proof {
        let mut proof = Proof::new(run_id);

        proof.gates = gate_results
            .iter()
            .map(|gr| ProofGate {
                name: gr.name.clone(),
                status: if gr.passed {
                    GateStatus::Passed
                } else {
                    GateStatus::Failed
                },
                required: gr.required,
                evidence: Some(serde_json::json!({
                    "command_line": gr.command_line.clone(),
                    "exit_code": gr.exit_code,
                    "timed_out": gr.timed_out,
                    "stdout_summary": gr.stdout_summary.clone(),
                    "stderr_summary": gr.stderr_summary.clone(),
                    "output_path": gr.output_path.clone(),
                    "timeout_secs": gr.timeout_secs,
                })),
            })
            .collect();

        proof.changed_files = changed_files
            .iter()
            .map(|p| ChangedFile {
                path: p.clone(),
                operation: "modified".to_string(),
            })
            .collect();

        proof.known_gaps = known_gaps.to_vec();

        let has_required_failure = proof
            .gates
            .iter()
            .any(|g| g.required && g.status == GateStatus::Failed);
        let has_required_gate = proof.gates.iter().any(|g| g.required);
        proof.status = if has_required_failure {
            ProofStatus::Failed
        } else if has_required_gate {
            ProofStatus::Ready
        } else {
            ProofStatus::NotReady
        };

        proof.summary = format!(
            "Run {}: {}. {} file(s) changed, {} gate(s), {} known gap(s).",
            proof.run_id,
            proof.status,
            proof.changed_files.len(),
            proof.gates.len(),
            proof.known_gaps.len(),
        );

        proof
    }
}
