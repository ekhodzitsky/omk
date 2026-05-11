use anyhow::Result;
use tracing::{info, warn};

use crate::runtime::events::{Event, EventBuilder, EventKind, TaskId, WorkerId};
use crate::runtime::scheduler::runner::TeamRunner;
use crate::runtime::scheduler::task::{Task, TaskState};
use crate::runtime::worker::WorkerSpec;

impl TeamRunner {
    /// Create a single seed task from a high-level description.
    pub fn seed_task(&mut self, description: &str) {
        let task = Task::new("task-1", "seed").with_description(description);
        self.claim_store.insert(task.clone());
        self.manifest.tasks.push(task);
    }

    /// Create multiple seed tasks from (task_id, description) pairs.
    pub fn seed_tasks(&mut self, descriptions: Vec<(String, String)>) {
        for (task_id, description) in descriptions {
            let task = Task::new(&task_id, "seed").with_description(&description);
            self.claim_store.insert(task.clone());
            self.manifest.tasks.push(task);
        }
    }

    /// Assign ready tasks to available workers.
    pub async fn dispatch_to_workers(&mut self, worker_specs: &[WorkerSpec]) -> Result<()> {
        let busy_workers: std::collections::HashSet<String> = self
            .claim_store
            .tasks()
            .values()
            .filter(|t| matches!(t.state, TaskState::Claimed | TaskState::Running))
            .filter_map(|t| t.owner.clone())
            .collect();

        let mut available_workers: Vec<&WorkerSpec> = worker_specs
            .iter()
            .filter(|w| !busy_workers.contains(&w.name))
            .collect();

        if available_workers.is_empty() {
            return Ok(());
        }

        let ready_ids: Vec<String> = {
            let ready = self.claim_store.ready_tasks();
            ready.iter().map(|t| t.id.clone()).collect()
        };

        for task_id in ready_ids {
            if available_workers.is_empty() {
                break;
            }

            let task = match self.claim_store.get(&task_id) {
                Some(t) => t.clone(),
                None => continue,
            };

            let mut assigned_idx = None;
            for (idx, worker) in available_workers.iter().enumerate() {
                let conflicts = self.ownership.would_conflict(&task, &worker.name);
                if conflicts.is_empty() {
                    assigned_idx = Some(idx);
                    break;
                }
                warn!(
                    task = %task.id,
                    worker = %worker.name,
                    conflicts = ?conflicts,
                    "Ownership conflict detected"
                );
            }

            let Some(idx) = assigned_idx else {
                warn!(
                    task = %task.id,
                    write_set = ?task.write_set,
                    "Task dispatch blocked by active ownership conflicts"
                );
                continue;
            };
            let worker = available_workers.remove(idx);

            if !self.claim_store.claim(&task_id, &worker.name) {
                continue;
            }

            if let Some(task_ref) = self.claim_store.get(&task_id) {
                self.ownership.register_task(task_ref);
            }

            let lease_secs = self
                .claim_store
                .tasks()
                .get(&task_id)
                .and_then(|t| t.lease_expires)
                .map(|dt| dt.timestamp() as u64)
                .unwrap_or(300);
            let claimed_event = EventBuilder::new(self.run_id.clone()).task_claimed(
                TaskId(task_id.clone()),
                WorkerId(worker.name.clone()),
                lease_secs,
            )?;
            self.event_writer.append(&claimed_event).await?;

            let started_event = Event::new(self.run_id.clone(), EventKind::TaskStarted)
                .with_actor(&worker.name)
                .with_payload(serde_json::json!({
                    "task_id": task_id,
                    "worker_id": worker.name,
                }))?;
            self.event_writer.append(&started_event).await?;
            self.claim_store.start(&task_id, &worker.name);

            let worker_task = crate::runtime::worker::WorkerTask {
                id: task_id.clone(),
                task: task.description.clone(),
                acceptance_criteria: Vec::new(),
                context: None,
            };
            worker.send_task(&worker_task).await?;

            info!(task = %task_id, worker = %worker.name, "Dispatched task to worker");
        }

        Ok(())
    }
}
