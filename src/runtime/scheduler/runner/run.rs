use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;

use crate::runtime::events::{Event, EventBuilder, EventKind, EventWriter, RunId};
use crate::runtime::scheduler::claim::ClaimStore;
use crate::runtime::scheduler::manifest::RunManifest;
use crate::runtime::scheduler::ownership::OwnershipMap;
use crate::runtime::scheduler::runner::{RunSummary, TeamRunner};
use crate::runtime::scheduler::task::Task;
use crate::runtime::worker::WorkerSpec;

impl TeamRunner {
    /// Initialize a new team run.
    pub async fn init(
        run_id: &str,
        task_desc: &str,
        project_dir: &Path,
        state_dir: &Path,
        event_writer: EventWriter,
    ) -> Result<Self> {
        let manifest = RunManifest::new(run_id, "team", project_dir).with_description(task_desc);
        manifest.init().await?;

        Ok(Self {
            manifest,
            claim_store: ClaimStore::new(),
            ownership: OwnershipMap::new(),
            event_writer,
            state_dir: state_dir.to_path_buf(),
            run_id: RunId(run_id.to_string()),
            last_outbox_offsets: HashMap::new(),
            last_heartbeat_ts: HashMap::new(),
        })
    }

    /// Initialize a new team run with a pre-built task list.
    pub async fn init_with_tasks(
        run_id: &str,
        project_dir: &Path,
        state_dir: &Path,
        event_writer: EventWriter,
        tasks: Vec<Task>,
    ) -> Result<Self> {
        let manifest = RunManifest::new(run_id, "team", project_dir).with_tasks(tasks.clone());
        manifest.init().await?;

        let mut claim_store = ClaimStore::new();
        for task in &tasks {
            claim_store.insert(task.clone());
        }

        Ok(Self {
            manifest,
            claim_store,
            ownership: OwnershipMap::new(),
            event_writer,
            state_dir: state_dir.to_path_buf(),
            run_id: RunId(run_id.to_string()),
            last_outbox_offsets: HashMap::new(),
            last_heartbeat_ts: HashMap::new(),
        })
    }

    /// Run the main loop until all tasks are done.
    pub async fn run(&mut self, worker_specs: &[WorkerSpec]) -> Result<RunSummary> {
        loop {
            self.dispatch_to_workers(worker_specs).await?;
            self.poll_workers().await?;

            let recovered = self.claim_store.recover_stale_leases();
            for task_id in &recovered {
                if let Some(task) = self.claim_store.get(task_id) {
                    self.ownership.release_task(task);
                }
                let event = Event::new(self.run_id.clone(), EventKind::RetryScheduled)
                    .with_actor("scheduler")
                    .with_message(format!("stale lease recovered for {}", task_id))?;
                self.event_writer.append(&event).await?;
            }

            self.snapshot().await?;

            if self.claim_store.all_done() {
                break;
            }

            tokio::time::sleep(std::time::Duration::from_secs(
                crate::runtime::scheduler::runner::RUNNER_POLL_INTERVAL_SECS,
            ))
            .await;
        }

        let summary = self.claim_store.summary();
        let success = summary.failed == 0 && summary.cancelled == 0;

        if success {
            let event = EventBuilder::new(self.run_id.clone()).run_completed();
            self.event_writer.append(&event).await?;
        } else {
            let event = EventBuilder::new(self.run_id.clone())
                .run_failed("one or more tasks failed or were cancelled")?;
            self.event_writer.append(&event).await?;
        }

        Ok(RunSummary {
            run_id: self.run_id.0.clone(),
            completed: summary.completed,
            failed: summary.failed,
            cancelled: summary.cancelled,
            total: summary.total(),
        })
    }

    /// Save current state to manifest.
    pub async fn snapshot(&self) -> Result<()> {
        let mut manifest = self.manifest.clone();
        manifest.tasks = self.claim_store.tasks().values().cloned().collect();
        manifest.save().await?;
        manifest.snapshot_tasks().await?;
        Ok(())
    }
}
