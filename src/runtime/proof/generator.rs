use std::collections::{BTreeSet, HashMap};
use std::path::Path;

use crate::runtime::events::{
    Event, EventKind, EventReader, FileChangedPayload, GateResultPayload, RunId,
    TaskCompletedPayload,
};
use crate::runtime::gates::GateResult;
use crate::runtime::proof::{
    ChangedFile, GateStatus, Proof, ProofFailure, ProofGate, ProofRetry, ProofStatus,
    WireEvidenceSummary,
};

/// Generate a proof from recorded events.
pub struct ProofGenerator;

impl ProofGenerator {
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

    pub fn from_event_list(run_id: &RunId, events: &[Event]) -> anyhow::Result<Proof> {
        let mut proof = Proof::new(run_id.clone());
        let mut file_changes: HashMap<String, String> = HashMap::new();
        let mut gate_results: Vec<ProofGate> = Vec::new();
        let mut failures: Vec<ProofFailure> = Vec::new();
        let mut retries: Vec<ProofRetry> = Vec::new();
        let mut known_gaps: Vec<String> = Vec::new();
        let mut command_evidence: HashMap<String, serde_json::Map<String, serde_json::Value>> =
            HashMap::new();
        let mut wire_events = 0usize;
        let mut wire_requests = 0usize;
        let mut wire_outputs = 0usize;
        let mut prompt_like_messages = 0usize;
        let mut unique_methods = BTreeSet::new();
        let mut unique_events = BTreeSet::new();
        let mut unique_requests = BTreeSet::new();

        let mut run_start: Option<chrono::DateTime<chrono::Utc>> = None;
        let mut run_end: Option<chrono::DateTime<chrono::Utc>> = None;

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
                            let evidence = gate_evidence_from_payload(payload).or_else(|| {
                                gate_key_from_payload(payload).and_then(|key| {
                                    command_evidence
                                        .get(&key)
                                        .map(|m| serde_json::Value::Object(m.clone()))
                                })
                            });
                            gate_results.push(ProofGate {
                                name: p.name,
                                status: GateStatus::Passed,
                                required: p.required,
                                evidence,
                            });
                        }
                    }
                }
                EventKind::GateFailed => {
                    if let Some(ref payload) = event.payload {
                        if let Ok(p) = serde_json::from_value::<GateResultPayload>(payload.clone())
                        {
                            let evidence = gate_evidence_from_payload(payload).or_else(|| {
                                gate_key_from_payload(payload).and_then(|key| {
                                    command_evidence
                                        .get(&key)
                                        .map(|m| serde_json::Value::Object(m.clone()))
                                })
                            });
                            gate_results.push(ProofGate {
                                name: p.name.clone(),
                                status: GateStatus::Failed,
                                required: p.required,
                                evidence,
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
                EventKind::TaskOutput | EventKind::TaskCompleted => {
                    if let Some(payload) = event.payload.as_ref() {
                        let wire_event = payload
                            .get("wire_event")
                            .or_else(|| payload.get("event_type"))
                            .or_else(|| payload.get("type"))
                            .and_then(value_as_string);
                        if let Some(wire_event) = wire_event {
                            wire_events += 1;
                            unique_events.insert(wire_event);
                        }

                        let wire_request = payload
                            .get("wire_request")
                            .or_else(|| payload.get("request_type"))
                            .or_else(|| payload.get("raw_request_type"))
                            .and_then(value_as_string);
                        if let Some(wire_request) = wire_request {
                            wire_requests += 1;
                            unique_requests.insert(wire_request);
                        }

                        if payload
                            .get("output_summary")
                            .and_then(value_as_string)
                            .is_some()
                        {
                            wire_outputs += 1;
                        }

                        if payload.get("message").and_then(value_as_string).is_some() {
                            prompt_like_messages += 1;
                        }

                        if let Some(method) = payload
                            .get("wire_method")
                            .or_else(|| payload.get("method"))
                            .and_then(value_as_string)
                        {
                            unique_methods.insert(method);
                        }
                    }
                }
                EventKind::CommandStarted => {
                    if let Some(payload) = event.payload.as_ref() {
                        let key = gate_key_from_payload(payload).unwrap_or_else(|| {
                            payload
                                .get("name")
                                .and_then(value_as_string)
                                .unwrap_or_default()
                        });
                        if !key.is_empty() {
                            let entry = command_evidence.entry(key).or_default();
                            copy_payload_field(payload, entry, "command_line");
                            copy_payload_field(payload, entry, "timeout_secs");
                        }
                    }
                }
                EventKind::CommandFinished => {
                    if let Some(payload) = event.payload.as_ref() {
                        let key = gate_key_from_payload(payload).unwrap_or_else(|| {
                            payload
                                .get("name")
                                .and_then(value_as_string)
                                .unwrap_or_default()
                        });
                        if !key.is_empty() {
                            let entry = command_evidence.entry(key).or_default();
                            if !entry.contains_key("command_line") {
                                if let Some(command) = payload.get("command") {
                                    entry.insert("command_line".to_string(), command.clone());
                                }
                            }
                            copy_payload_field(payload, entry, "exit_code");
                            copy_payload_field(payload, entry, "timed_out");
                            copy_payload_field(payload, entry, "stdout_summary");
                            copy_payload_field(payload, entry, "stderr_summary");
                            copy_payload_field(payload, entry, "output_path");
                        }
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
        if wire_events > 0 || wire_requests > 0 || wire_outputs > 0 || prompt_like_messages > 0 {
            proof.wire_evidence = Some(WireEvidenceSummary {
                event_count: wire_events,
                request_count: wire_requests,
                output_count: wire_outputs,
                prompt_like_messages,
                unique_methods: unique_methods.into_iter().collect(),
                unique_events: unique_events.into_iter().collect(),
                unique_requests: unique_requests.into_iter().collect(),
            });
        }

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

pub(crate) fn value_as_string(value: &serde_json::Value) -> Option<String> {
    if let Some(text) = value.as_str() {
        return Some(text.to_string());
    }
    if let Some(number) = value.as_i64() {
        return Some(number.to_string());
    }
    if let Some(number) = value.as_u64() {
        return Some(number.to_string());
    }
    if let Some(number) = value.as_f64() {
        return Some(number.to_string());
    }
    if let Some(boolean) = value.as_bool() {
        return Some(boolean.to_string());
    }
    value.get("0")?.as_str().map(str::to_string)
}

pub(crate) fn gate_evidence_from_payload(payload: &serde_json::Value) -> Option<serde_json::Value> {
    let mut evidence = serde_json::Map::new();
    for key in [
        "command_line",
        "exit_code",
        "timed_out",
        "stdout_summary",
        "stderr_summary",
        "output_path",
        "timeout_secs",
    ] {
        if let Some(value) = payload.get(key) {
            evidence.insert(key.to_string(), value.clone());
        }
    }
    if evidence.is_empty() {
        None
    } else {
        Some(serde_json::Value::Object(evidence))
    }
}

pub(crate) fn gate_key_from_payload(payload: &serde_json::Value) -> Option<String> {
    payload
        .get("gate_id")
        .and_then(value_as_string)
        .or_else(|| payload.get("name").and_then(value_as_string))
}

pub(crate) fn copy_payload_field(
    payload: &serde_json::Value,
    into: &mut serde_json::Map<String, serde_json::Value>,
    key: &str,
) {
    if let Some(value) = payload.get(key) {
        into.insert(key.to_string(), value.clone());
    }
}
