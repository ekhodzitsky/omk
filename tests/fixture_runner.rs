use std::path::Path;

use omk::runtime::events::{Event, EventBuilder, EventWriter, RunId, TaskId, WorkerId};
use omk::runtime::proof::{Proof, ProofGenerator};

pub struct FixtureRunner {
    pub run_id: RunId,
    pub event_writer: EventWriter,
    pub events_path: std::path::PathBuf,
}

impl FixtureRunner {
    pub fn new(dir: &Path, run_id: &str) -> Self {
        let events_path = dir.join("events.jsonl");
        let event_writer = EventWriter::new(&events_path);
        Self {
            run_id: RunId(run_id.to_string()),
            event_writer,
            events_path,
        }
    }

    pub async fn emit_run_started(&self, mode: &str, project_dir: &Path, task: &str) {
        let event = EventBuilder::new(self.run_id.clone())
            .run_started(mode, project_dir, task)
            .unwrap();
        self.event_writer.append(&event).await.unwrap();
    }

    pub async fn emit_worker_started(&self, worker_id: &str, role: &str) {
        let event = EventBuilder::new(self.run_id.clone())
            .worker_started(WorkerId(worker_id.to_string()), role)
            .unwrap();
        self.event_writer.append(&event).await.unwrap();
    }

    pub async fn emit_task_completed(&self, task_id: &str, worker_id: &str, summary: &str) {
        let event = EventBuilder::new(self.run_id.clone())
            .task_completed(
                TaskId(task_id.to_string()),
                WorkerId(worker_id.to_string()),
                Some(summary),
            )
            .unwrap();
        self.event_writer.append(&event).await.unwrap();
    }

    pub async fn emit_gate_passed(&self, gate_name: &str) {
        let event = EventBuilder::new(self.run_id.clone())
            .gate_passed_by_name(gate_name)
            .unwrap();
        self.event_writer.append(&event).await.unwrap();
    }

    pub async fn emit_gate_failed(&self, gate_name: &str, _error: &str) {
        let event = EventBuilder::new(self.run_id.clone())
            .gate_failed_by_name(gate_name)
            .unwrap();
        self.event_writer.append(&event).await.unwrap();
    }

    pub async fn emit_run_completed(&self) {
        let event = EventBuilder::new(self.run_id.clone()).run_completed();
        self.event_writer.append(&event).await.unwrap();
    }

    pub async fn emit_file_changed(&self, path: &str, change_type: &str) {
        let event = EventBuilder::new(self.run_id.clone())
            .file_changed(path, change_type)
            .unwrap();
        self.event_writer.append(&event).await.unwrap();
    }

    pub fn generate_proof(&self) -> Proof {
        let content = std::fs::read_to_string(&self.events_path).unwrap();
        let events: Vec<Event> = content
            .lines()
            .filter_map(|line| {
                let line = line.trim();
                if line.is_empty() {
                    return None;
                }
                serde_json::from_str(line).ok()
            })
            .collect();
        ProofGenerator::from_event_list(&self.run_id, &events).unwrap()
    }
}
