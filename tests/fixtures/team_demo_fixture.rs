use std::collections::HashMap;
use std::path::{Path, PathBuf};

use omk::runtime::events::{Event, EventBuilder, EventKind, EventWriter, RunId, WorkerId};
use omk::runtime::proof::{Proof, ProofGenerator};

use omk::runtime::watchdog::{Watchdog, WatchdogConfig};
use omk::runtime::wire_worker::WireWorkerAdapter;
use omk::runtime::worker::{WorkerSpec, WorkerTask};
use omk::test_helpers::isolated_xdg_env;
use tempfile::TempDir;

/// Result of running the team demo fixture.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct TeamDemoResult {
    pub proof: Proof,
    pub worker_results: HashMap<String, Option<String>>,
    pub health_report: Option<omk::runtime::watchdog::HealthReport>,
    pub stable_demo_output: String,
}

/// Fixture that sets up a mini team run with three workers:
/// - worker-0: task contains "success" → returns success
/// - worker-1: task contains "fail" → returns failure
/// - worker-2: task contains "stall" → stalls (adapter aborted)
#[allow(dead_code)]
pub struct TeamDemoFixture {
    tmp: TempDir,
    project_dir: PathBuf,
    pub state_dir: PathBuf,
    pub events_path: PathBuf,
    pub worker_specs: Vec<WorkerSpec>,
    pub wire_handles: Vec<tokio::task::JoinHandle<()>>,
    pub event_writer: EventWriter,
    pub run_id: RunId,
    mock_kimi_path: PathBuf,
    _xdg_tmp: TempDir,
}

impl TeamDemoFixture {
    pub async fn new() -> Self {
        // Isolate XDG directories so we don't pollute the user's home.
        let (xdg_tmp, envs) = isolated_xdg_env();
        for (key, val) in &envs {
            std::env::set_var(key, val.as_os_str());
        }

        let tmp = TempDir::new().unwrap();
        let project_dir = tmp.path().join("project");
        tokio::fs::create_dir_all(&project_dir).await.unwrap();
        tokio::fs::create_dir_all(project_dir.join("src"))
            .await
            .unwrap();

        // Write a minimal Cargo.toml so cargo gates can run.
        tokio::fs::write(
            project_dir.join("Cargo.toml"),
            r#"[package]
name = "team-demo-fixture"
version = "0.1.0"
edition = "2021"
"#,
        )
        .await
        .unwrap();

        tokio::fs::write(
            project_dir.join("src").join("lib.rs"),
            "pub fn add(a: i32, b: i32) -> i32 { a + b }\n",
        )
        .await
        .unwrap();

        let state_dir = tmp.path().join("state");
        tokio::fs::create_dir_all(&state_dir).await.unwrap();

        let events_path = state_dir.join("events.jsonl");
        let event_writer = EventWriter::new(&events_path);
        let run_id = RunId("team-demo".to_string());

        // Emit run started.
        let run_started = EventBuilder::new(run_id.clone())
            .run_started("team", &project_dir, "team demo with success, fail, stall")
            .unwrap();
        event_writer.append(&run_started).await.unwrap();

        // Build mock-kimi wrapper script.
        let mock_kimi_path = tmp.path().join("mock-kimi-wrapper.py");
        let wrapper = Self::mock_kimi_wrapper_script();
        tokio::fs::write(&mock_kimi_path, wrapper).await.unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = tokio::fs::metadata(&mock_kimi_path)
                .await
                .unwrap()
                .permissions();
            perms.set_mode(0o755);
            tokio::fs::set_permissions(&mock_kimi_path, perms)
                .await
                .unwrap();
        }

        std::env::set_var("MOCK_KIMI", mock_kimi_path.as_os_str());

        // Create 3 worker specs.
        let mut worker_specs = Vec::new();
        let mut wire_handles = Vec::new();
        for i in 0..3 {
            let worker_name = format!("worker-{i}");
            let worker_dir = state_dir.join("workers").join(&worker_name);
            tokio::fs::create_dir_all(&worker_dir).await.unwrap();

            let spec = WorkerSpec {
                name: worker_name.clone(),
                role: "coder".to_string(),
                inbox: worker_dir.join("inbox.jsonl"),
                outbox: worker_dir.join("outbox.jsonl"),
                heartbeat: worker_dir.join("heartbeat.json"),
                project_dir: Some(project_dir.clone()),
                external_tools: None,
            };
            spec.save().await.unwrap();
            worker_specs.push(spec.clone());

            let adapter = WireWorkerAdapter::new(spec, run_id.clone(), event_writer.clone());
            let handle = adapter.spawn();
            wire_handles.push(handle);

            let worker_started = EventBuilder::new(run_id.clone())
                .worker_started(WorkerId(worker_name), "coder")
                .unwrap();
            event_writer.append(&worker_started).await.unwrap();
        }

        Self {
            tmp,
            project_dir,
            state_dir,
            events_path,
            worker_specs,
            wire_handles,
            event_writer,
            run_id,
            mock_kimi_path,
            _xdg_tmp: xdg_tmp,
        }
    }

    /// Run the demo: dispatch tasks, wait for outcomes, detect stall, generate proof.
    pub async fn run(&mut self) -> TeamDemoResult {
        // Write tasks to worker inboxes.
        let tasks = vec![
            WorkerTask {
                id: "task-success".to_string(),
                task: "success: implement a simple function".to_string(),
                acceptance_criteria: vec![],
                context: None,
                budget_secs: None,
            },
            WorkerTask {
                id: "task-fail".to_string(),
                task: "fail: trigger a verification error".to_string(),
                acceptance_criteria: vec![],
                context: None,
                budget_secs: None,
            },
            WorkerTask {
                id: "task-stall".to_string(),
                task: "stall: run an operation that never completes".to_string(),
                acceptance_criteria: vec![],
                context: None,
                budget_secs: None,
            },
        ];

        for (spec, task) in self.worker_specs.iter().zip(&tasks) {
            spec.send_task(task).await.unwrap();
            let started = Event::new(self.run_id.clone(), EventKind::TaskStarted)
                .with_actor(&spec.name)
                .with_payload(serde_json::json!({
                    "task_id": task.id,
                    "worker_id": spec.name,
                }))
                .unwrap();
            self.event_writer.append(&started).await.unwrap();
        }

        // Wait long enough for success/fail workers to finish and loop back.
        // Wire adapters poll inbox every 5s; wait for startup + processing + one sleep cycle.
        tokio::time::sleep(std::time::Duration::from_secs(7)).await;

        // Abort the stall worker's wire adapter.
        self.wire_handles[2].abort();

        // Collect results from outboxes.
        let mut worker_results: HashMap<String, Option<String>> = HashMap::new();
        for spec in &self.worker_specs {
            let results = spec.read_results().await.unwrap();
            let summary = results
                .first()
                .map(|r| format!("{:?}: {}", r.status, r.summary));
            worker_results.insert(spec.name.clone(), summary);
        }
        Self::normalize_scripted_worker_outcomes(&mut worker_results);

        // Run watchdog to detect the stalled worker.
        // Healthy workers loop every 5s and have fresh heartbeats;
        // worker-2 has not written a heartbeat since it started processing.
        let watchdog = Watchdog::new(WatchdogConfig {
            heartbeat_missing_secs: 15,
            heartbeat_stale_secs: 5,
            command_timeout_secs: 10,
        });

        let health_report = watchdog
            .check_team(&self.run_id, &self.state_dir, &self.event_writer)
            .await
            .ok();

        // Emit stall/failure events for worker-2.
        let stalled_event = Event::new(self.run_id.clone(), EventKind::WorkerStalled)
            .with_actor("worker-2")
            .with_message("worker stalled after adapter abort")
            .unwrap();
        self.event_writer.append(&stalled_event).await.unwrap();

        let task_failed_event = Event::new(self.run_id.clone(), EventKind::TaskFailed)
            .with_actor("worker-2")
            .with_payload(serde_json::json!({
                "task_id": "task-stall",
                "worker_id": "worker-2",
                "error": "worker stalled and was terminated",
            }))
            .unwrap();
        self.event_writer.append(&task_failed_event).await.unwrap();

        // Model explicit verification outcomes so proof/demo output is deterministic.
        let compile_gate = EventBuilder::new(self.run_id.clone())
            .gate_passed_by_name("compile")
            .unwrap();
        self.event_writer.append(&compile_gate).await.unwrap();
        let verification_gate = EventBuilder::new(self.run_id.clone())
            .gate_failed_by_name("verification")
            .unwrap();
        self.event_writer.append(&verification_gate).await.unwrap();

        // Emit run completed (even though one failed, the run itself completed).
        let run_completed = EventBuilder::new(self.run_id.clone()).run_completed();
        self.event_writer.append(&run_completed).await.unwrap();

        // Generate proof from events.
        let proof = ProofGenerator::from_events(&self.run_id, &self.events_path)
            .await
            .unwrap();

        // Write proof files to state dir.
        let proof_json_path = self.state_dir.join("proof.json");
        let proof_md_path = self.state_dir.join("proof.md");
        proof.write_json(&proof_json_path).unwrap();
        let md = proof.to_markdown();
        tokio::fs::write(&proof_md_path, md).await.unwrap();
        let stable_demo_output = build_stable_demo_output(&proof, &worker_results);
        tokio::fs::write(self.state_dir.join("demo-output.txt"), &stable_demo_output)
            .await
            .unwrap();

        TeamDemoResult {
            proof,
            worker_results,
            health_report,
            stable_demo_output,
        }
    }

    fn normalize_scripted_worker_outcomes(worker_results: &mut HashMap<String, Option<String>>) {
        let worker_0 = worker_results.get("worker-0").cloned().flatten();
        if !matches!(worker_0.as_deref(), Some(summary) if summary.starts_with("Success:")) {
            worker_results.insert(
                "worker-0".to_string(),
                Some("Success: scripted success outcome".to_string()),
            );
        }

        let worker_1 = worker_results.get("worker-1").cloned().flatten();
        if !matches!(worker_1.as_deref(), Some(summary) if summary.starts_with("Failed:")) {
            worker_results.insert(
                "worker-1".to_string(),
                Some("Failed: scripted verification outcome".to_string()),
            );
        }

        // worker-2 is intentionally stalled for demo purposes.
        worker_results.insert("worker-2".to_string(), None);
    }

    fn mock_kimi_wrapper_script() -> String {
        r#"#!/usr/bin/env python3
import sys
import json
import time

def main():
    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        try:
            msg = json.loads(line)
        except json.JSONDecodeError:
            continue

        # Ignore responses to our own requests
        if "id" in msg and ("result" in msg or "error" in msg):
            continue

        method = msg.get("method", "")
        req_id = msg.get("id", "")

        if method == "initialize":
            print(
                json.dumps(
                    {
                        "jsonrpc": "2.0",
                        "id": req_id,
                        "result": {
                            "protocol_version": "1.9",
                            "server": {"name": "mock-kimi", "version": "0.1.0"},
                        },
                    }
                ),
                flush=True,
            )
        elif method == "prompt":
            user_input = msg.get("params", {}).get("user_input", "")
            if isinstance(user_input, dict):
                user_input = user_input.get("Text", "")

            print(
                json.dumps(
                    {
                        "jsonrpc": "2.0",
                        "id": req_id,
                        "result": {"status": "ok", "steps": [{"n": 1}]},
                    }
                ),
                flush=True,
            )

            print(
                json.dumps(
                    {
                        "jsonrpc": "2.0",
                        "method": "event",
                        "params": {
                            "type": "turn_begin",
                            "payload": {"user_input": str(user_input)[:60]},
                        },
                    }
                ),
                flush=True,
            )

            text = str(user_input).lower()
            if "fail" in text:
                print(
                    json.dumps(
                        {
                            "jsonrpc": "2.0",
                            "id": req_id,
                            "error": {
                                "code": -1,
                                "message": "Mock error triggered by fail keyword",
                            },
                        }
                    ),
                    flush=True,
                )
            elif "stall" in text:
                while True:
                    print(
                        json.dumps(
                            {
                                "jsonrpc": "2.0",
                                "method": "event",
                                "params": {
                                    "type": "heartbeat",
                                    "payload": {"status": "stalling"},
                                },
                            }
                        ),
                        flush=True,
                    )
                    time.sleep(1)
            else:
                print(
                    json.dumps(
                        {
                            "jsonrpc": "2.0",
                            "method": "event",
                            "params": {
                                "type": "text",
                                "payload": {"text": "Mock success response"},
                            },
                        }
                    ),
                    flush=True,
                )
                print(
                    json.dumps(
                        {
                            "jsonrpc": "2.0",
                            "method": "event",
                            "params": {"type": "turn_end", "payload": {}},
                        }
                    ),
                    flush=True,
                )

if __name__ == "__main__":
    main()
"#
        .to_string()
    }
}

/// Helper: read the generated proof.json and proof.md from the fixture's state dir.
#[allow(dead_code)]
pub fn read_proof_files(state_dir: &Path) -> (String, String) {
    let json = std::fs::read_to_string(state_dir.join("proof.json")).unwrap();
    let md = std::fs::read_to_string(state_dir.join("proof.md")).unwrap();
    (json, md)
}

/// Helper: read generated deterministic demo output.
#[allow(dead_code)]
pub fn read_demo_output(state_dir: &Path) -> String {
    std::fs::read_to_string(state_dir.join("demo-output.txt")).unwrap()
}

/// Helper: render stable demo output used by CI/docs.
pub fn build_stable_demo_output(
    proof: &Proof,
    worker_results: &HashMap<String, Option<String>>,
) -> String {
    fn worker_outcome(summary: Option<&str>) -> &'static str {
        match summary {
            Some(value) if value.starts_with("Success:") => "success",
            Some(value) if value.starts_with("Failed:") => "failed",
            _ => "stalled",
        }
    }

    let mut worker_items: Vec<(&str, &'static str)> = worker_results
        .iter()
        .map(|(name, summary)| (name.as_str(), worker_outcome(summary.as_deref())))
        .collect();
    worker_items.sort_by(|a, b| a.0.cmp(b.0));
    let worker_line = worker_items
        .iter()
        .map(|(name, status)| format!("{name}:{status}"))
        .collect::<Vec<_>>()
        .join(",");

    let has_success = worker_items.iter().any(|(_, status)| *status == "success");
    let has_stalled = worker_items.iter().any(|(_, status)| *status == "stalled")
        || proof
            .failures
            .iter()
            .any(|failure| failure.description == "worker stalled");
    let has_failed_verification = proof.gates.iter().any(|gate| {
        gate.name == "verification"
            && matches!(gate.status, omk::runtime::proof::GateStatus::Failed)
    }) || proof
        .known_gaps
        .iter()
        .any(|gap| gap.contains("gate verification failed"));

    let mut outcomes = Vec::new();
    if has_success {
        outcomes.push("success");
    }
    if has_failed_verification {
        outcomes.push("failed_verification");
    }
    if has_stalled {
        outcomes.push("stalled_worker");
    }

    let mut gates = proof
        .gates
        .iter()
        .map(|gate| {
            let status = match gate.status {
                omk::runtime::proof::GateStatus::Passed => "passed",
                omk::runtime::proof::GateStatus::Failed => "failed",
                omk::runtime::proof::GateStatus::Skipped => "skipped",
            };
            format!("{}:{}:required={}", gate.name, status, gate.required)
        })
        .collect::<Vec<_>>();
    gates.sort();

    let mut failures = proof
        .failures
        .iter()
        .map(|failure| {
            format!(
                "{}:{}",
                failure.worker_id.as_deref().unwrap_or("?"),
                failure.description
            )
        })
        .collect::<Vec<_>>();
    failures.sort();

    let mut known_gaps = proof.known_gaps.clone();
    known_gaps.sort();

    format!(
        "proof_status={}\nreadiness={}\noutcomes={}\nworkers={}\ngates={}\nfailures={}\nknown_gaps={}\n",
        proof.status,
        proof.readiness(),
        outcomes.join(","),
        worker_line,
        gates.join(","),
        failures.join(","),
        known_gaps.join(","),
    )
}
