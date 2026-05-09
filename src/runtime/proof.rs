use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::info;

use super::events::{
    Event, EventKind, EventReader, FileChangedPayload, GateResultPayload, RunId,
    TaskCompletedPayload,
};
use super::gates::GateResult;

/// Final readiness report for a run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Proof {
    pub run_id: RunId,
    pub status: ProofStatus,
    pub generated_at: DateTime<Utc>,
    pub changed_files: Vec<ChangedFile>,
    pub gates: Vec<ProofGate>,
    pub failures: Vec<ProofFailure>,
    pub retries: Vec<ProofRetry>,
    pub known_gaps: Vec<String>,
    pub summary: String,
    pub elapsed_secs: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProofStatus {
    Ready,
    NotReady,
    Failed,
}

impl std::fmt::Display for ProofStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProofStatus::Ready => write!(f, "ready"),
            ProofStatus::NotReady => write!(f, "not_ready"),
            ProofStatus::Failed => write!(f, "failed"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangedFile {
    pub path: String,
    pub operation: String, // "created", "modified", "deleted"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProofGate {
    pub name: String,
    pub status: GateStatus,
    pub required: bool,
    pub evidence: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GateStatus {
    Passed,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProofFailure {
    pub task_id: Option<String>,
    pub worker_id: Option<String>,
    pub description: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProofRetry {
    pub task_id: String,
    pub attempt: u32,
    pub reason: String,
}

impl Proof {
    pub fn new(run_id: RunId) -> Self {
        Self {
            run_id,
            status: ProofStatus::NotReady,
            generated_at: Utc::now(),
            changed_files: Vec::new(),
            gates: Vec::new(),
            failures: Vec::new(),
            retries: Vec::new(),
            known_gaps: Vec::new(),
            summary: String::new(),
            elapsed_secs: 0,
        }
    }

    pub fn proof_path(state_dir: &Path) -> PathBuf {
        state_dir.join("proof.json")
    }

    pub fn write_json(&self, path: &Path) -> Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    pub async fn save(&self, state_dir: &Path) -> Result<()> {
        let path = Self::proof_path(state_dir);
        let json = serde_json::to_string_pretty(self)?;
        crate::runtime::atomic::atomic_write(&path, json.as_bytes()).await?;
        info!(path = %path.display(), status = %self.status, "Saved proof");
        Ok(())
    }

    pub async fn load(state_dir: &Path) -> Result<Option<Self>> {
        let path = Self::proof_path(state_dir);
        if !path.exists() {
            return Ok(None);
        }
        let json = tokio::fs::read_to_string(&path).await?;
        let proof: Self = serde_json::from_str(&json)?;
        Ok(Some(proof))
    }

    pub fn readiness(&self) -> &'static str {
        match self.status {
            ProofStatus::Ready => "ready_for_handoff",
            ProofStatus::NotReady => "needs_follow_up",
            ProofStatus::Failed => "blocked",
        }
    }

    pub fn gate_counts(&self) -> (usize, usize, usize) {
        (
            self.gates
                .iter()
                .filter(|g| g.status == GateStatus::Passed)
                .count(),
            self.gates
                .iter()
                .filter(|g| g.status == GateStatus::Failed)
                .count(),
            self.gates
                .iter()
                .filter(|g| g.status == GateStatus::Skipped)
                .count(),
        )
    }

    pub fn to_markdown(&self) -> String {
        let mut md = String::new();
        let (gates_passed, gates_failed, gates_skipped) = self.gate_counts();

        md.push_str(&format!("# Proof Report for {}\n\n", self.run_id));
        md.push_str(&format!("**Status:** {}  \n", self.status));
        md.push_str(&format!("**Readiness:** {}  \n", self.readiness()));
        md.push_str(&format!("**Generated:** {}  \n", self.generated_at));
        if self.elapsed_secs > 0 {
            md.push_str(&format!("**Duration:** {}s  \n", self.elapsed_secs));
        }
        md.push('\n');

        md.push_str("## Verdict\n\n");
        md.push_str(&format!("- status: `{}`\n", self.status));
        md.push_str(&format!("- readiness: `{}`\n", self.readiness()));
        md.push_str(&format!(
            "- changed_files: `{}`\n",
            self.changed_files.len()
        ));
        md.push_str(&format!("- gates_total: `{}`\n", self.gates.len()));
        md.push_str(&format!(
            "- gates: passed=`{}`, failed=`{}`, skipped=`{}`\n",
            gates_passed, gates_failed, gates_skipped
        ));
        md.push_str(&format!("- failures: `{}`\n", self.failures.len()));
        md.push_str(&format!("- retries: `{}`\n", self.retries.len()));
        md.push_str(&format!("- known_gaps: `{}`\n\n", self.known_gaps.len()));

        md.push_str("## Changed Files\n\n");
        if self.changed_files.is_empty() {
            md.push_str("_none_\n\n");
        } else {
            md.push_str("| Operation | Path |\n");
            md.push_str("|-----------|------|\n");
            for f in &self.changed_files {
                md.push_str(&format!("| {} | {} |\n", f.operation, f.path));
            }
            md.push('\n');
        }

        md.push_str("## Gates\n\n");
        if self.gates.is_empty() {
            md.push_str("_none_\n\n");
        } else {
            md.push_str("| Gate | Status | Required |\n");
            md.push_str("|------|--------|----------|\n");
            for g in &self.gates {
                let status_str = match g.status {
                    GateStatus::Passed => "passed",
                    GateStatus::Failed => "failed",
                    GateStatus::Skipped => "skipped",
                };
                let required_str = if g.required { "Yes" } else { "No" };
                md.push_str(&format!(
                    "| {} | {} | {} |\n",
                    g.name, status_str, required_str
                ));
            }
            md.push('\n');
        }

        md.push_str("## Failures\n\n");
        if self.failures.is_empty() {
            md.push_str("_none_\n\n");
        } else {
            for f in &self.failures {
                md.push_str(&format!(
                    "- **{}** — {}\n",
                    f.worker_id.as_deref().unwrap_or("?"),
                    f.description
                ));
            }
            md.push('\n');
        }

        md.push_str("## Retries\n\n");
        if self.retries.is_empty() {
            md.push_str("_none_\n\n");
        } else {
            for r in &self.retries {
                md.push_str(&format!(
                    "- **{}** (attempt {}): {}\n",
                    r.task_id, r.attempt, r.reason
                ));
            }
            md.push('\n');
        }

        md.push_str("## Known Gaps\n\n");
        if self.known_gaps.is_empty() {
            md.push_str("_none_\n\n");
        } else {
            for g in &self.known_gaps {
                md.push_str(&format!("- {}\n", g));
            }
            md.push('\n');
        }

        md.push_str("## Summary\n\n");
        md.push_str(&format!("{}\n\n", self.summary));
        md.push_str("---\n\n");
        md.push_str(&format!("Readiness verdict: `{}`.\n", self.readiness()));

        md
    }
}

/// Generate a proof from recorded events.
pub struct ProofGenerator;

impl ProofGenerator {
    pub async fn from_events(run_id: &RunId, event_log: &Path) -> Result<Proof> {
        let events = EventReader::read_all(event_log).await?;
        Self::from_event_list(run_id, &events)
    }

    pub fn from_event_list(run_id: &RunId, events: &[Event]) -> Result<Proof> {
        let mut proof = Proof::new(run_id.clone());
        let mut file_changes: HashMap<String, String> = HashMap::new();
        let mut gate_results: Vec<ProofGate> = Vec::new();
        let mut failures: Vec<ProofFailure> = Vec::new();
        let mut retries: Vec<ProofRetry> = Vec::new();
        let mut known_gaps: Vec<String> = Vec::new();

        let mut run_start: Option<DateTime<Utc>> = None;
        let mut run_end: Option<DateTime<Utc>> = None;

        for event in events {
            match &event.kind {
                EventKind::RunStarted => {
                    run_start = Some(event.ts);
                }
                EventKind::RunCompleted => {
                    run_end = Some(event.ts);
                }
                EventKind::RunFailed => {
                    run_end = Some(event.ts);
                    failures.push(ProofFailure {
                        task_id: None,
                        worker_id: event.actor.clone(),
                        description: event
                            .payload
                            .as_ref()
                            .and_then(|p| p.get("reason").and_then(|r| r.as_str()))
                            .unwrap_or("run failed")
                            .to_string(),
                        timestamp: event.ts,
                    });
                }
                EventKind::TaskFailed => {
                    if let Some(ref payload) = event.payload {
                        if let Ok(p) =
                            serde_json::from_value::<TaskCompletedPayload>(payload.clone())
                        {
                            failures.push(ProofFailure {
                                task_id: Some(p.task_id.0),
                                worker_id: Some(p.worker_id.0),
                                description: "task failed".to_string(),
                                timestamp: event.ts,
                            });
                        }
                    }
                }
                EventKind::FileChanged => {
                    if let Some(ref payload) = event.payload {
                        if let Ok(p) = serde_json::from_value::<FileChangedPayload>(payload.clone())
                        {
                            file_changes.insert(p.path.clone(), p.operation.clone());
                        }
                    }
                }
                EventKind::GatePassed => {
                    if let Some(ref payload) = event.payload {
                        if let Ok(p) = serde_json::from_value::<GateResultPayload>(payload.clone())
                        {
                            gate_results.push(ProofGate {
                                name: p.name,
                                status: GateStatus::Passed,
                                required: p.required,
                                evidence: None,
                            });
                        }
                    }
                }
                EventKind::GateFailed => {
                    if let Some(ref payload) = event.payload {
                        if let Ok(p) = serde_json::from_value::<GateResultPayload>(payload.clone())
                        {
                            gate_results.push(ProofGate {
                                name: p.name.clone(),
                                status: GateStatus::Failed,
                                required: p.required,
                                evidence: None,
                            });
                            if p.required {
                                failures.push(ProofFailure {
                                    task_id: None,
                                    worker_id: event.actor.clone(),
                                    description: format!("gate {} failed", p.name),
                                    timestamp: event.ts,
                                });
                            }
                            known_gaps.push(format!("gate {} failed", p.name));
                        }
                    }
                }
                EventKind::RetryScheduled => {
                    if let Some(ref payload) = event.payload {
                        let task_id = payload
                            .get("task_id")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown")
                            .to_string();
                        let attempt =
                            payload.get("attempt").and_then(|v| v.as_u64()).unwrap_or(1) as u32;
                        retries.push(ProofRetry {
                            task_id,
                            attempt,
                            reason: payload
                                .get("reason")
                                .and_then(|v| v.as_str())
                                .unwrap_or("retry")
                                .to_string(),
                        });
                    }
                }
                EventKind::ManualInterrupt => {
                    failures.push(ProofFailure {
                        task_id: None,
                        worker_id: event.actor.clone(),
                        description: "manual interrupt".to_string(),
                        timestamp: event.ts,
                    });
                }
                EventKind::WorkerStalled => {
                    failures.push(ProofFailure {
                        task_id: None,
                        worker_id: event.actor.clone(),
                        description: "worker stalled".to_string(),
                        timestamp: event.ts,
                    });
                }
                _ => {}
            }
        }

        // Deduplicate file changes (last operation wins)
        proof.changed_files = file_changes
            .into_iter()
            .map(|(path, operation)| ChangedFile { path, operation })
            .collect();

        proof.gates = gate_results;
        proof.failures = failures;
        proof.retries = retries;
        proof.known_gaps = known_gaps;

        // Compute elapsed time
        if let (Some(start), Some(end)) = (run_start, run_end) {
            proof.elapsed_secs = end.signed_duration_since(start).num_seconds().max(0) as u64;
        }

        // Determine status
        let has_required_gate_failure = proof
            .gates
            .iter()
            .any(|g| g.required && g.status == GateStatus::Failed);
        let has_failure = !proof.failures.is_empty();
        let has_required_gate = proof.gates.iter().any(|g| g.required);
        let required_gates_passed = proof
            .gates
            .iter()
            .filter(|g| g.required)
            .all(|g| g.status == GateStatus::Passed);

        proof.status = if has_required_gate_failure || has_failure {
            ProofStatus::Failed
        } else if has_required_gate && required_gates_passed {
            ProofStatus::Ready
        } else {
            ProofStatus::NotReady
        };

        // Build summary
        proof.summary = format!(
            "Run {}: {}. {} file(s) changed, {} gate(s) ({} passed, {} failed, {} skipped), {} failure(s), {} retry(ies).",
            proof.run_id,
            proof.status,
            proof.changed_files.len(),
            proof.gates.len(),
            proof.gates.iter().filter(|g| g.status == GateStatus::Passed).count(),
            proof.gates.iter().filter(|g| g.status == GateStatus::Failed).count(),
            proof.gates.iter().filter(|g| g.status == GateStatus::Skipped).count(),
            proof.failures.len(),
            proof.retries.len(),
        );

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
                evidence: Some(format!(
                    "stdout: {}...",
                    &gr.stdout.chars().take(200).collect::<String>()
                )),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::events::{EventBuilder, WorkerId};

    #[test]
    fn proof_from_gate_results() {
        let gate_results = vec![
            GateResult {
                name: "fmt".to_string(),
                passed: true,
                stdout: "ok".to_string(),
                stderr: "".to_string(),
                duration_ms: 100,
                required: true,
            },
            GateResult {
                name: "test".to_string(),
                passed: false,
                stdout: "failed".to_string(),
                stderr: "".to_string(),
                duration_ms: 200,
                required: true,
            },
        ];

        let proof = ProofGenerator::from_gate_results(
            RunId("run-1".to_string()),
            &gate_results,
            &["src/main.rs".to_string()],
            &["docs".to_string()],
        );

        assert_eq!(proof.status, ProofStatus::Failed);
        assert_eq!(proof.gates.len(), 2);
        assert_eq!(proof.changed_files.len(), 1);
    }

    #[tokio::test]
    async fn proof_from_events() {
        let tmp = tempfile::tempdir().unwrap();
        let event_log = tmp.path().join("events.jsonl");
        let writer = super::super::events::EventWriter::new(&event_log);
        let run_id = RunId("run-test".to_string());
        let builder = EventBuilder::new(run_id.clone());

        let events = vec![
            builder.run_started("team", tmp.path(), "test").unwrap(),
            builder
                .worker_started(WorkerId("w1".to_string()), "coder")
                .unwrap(),
            builder.file_changed("src/main.rs", "modified").unwrap(),
            builder
                .gate_passed(super::super::events::GateId("g1".to_string()), "fmt", true)
                .unwrap(),
            builder
                .gate_failed(super::super::events::GateId("g2".to_string()), "test", true)
                .unwrap(),
            builder.run_completed(),
        ];

        for e in &events {
            writer.append(e).await.unwrap();
        }

        let proof = ProofGenerator::from_events(&run_id, &event_log)
            .await
            .unwrap();
        assert_eq!(proof.status, ProofStatus::Failed);
        assert_eq!(proof.changed_files.len(), 1);
        assert_eq!(proof.gates.len(), 2);
        assert_eq!(proof.gates[0].status, GateStatus::Passed);
        assert_eq!(proof.gates[1].status, GateStatus::Failed);
    }
}
