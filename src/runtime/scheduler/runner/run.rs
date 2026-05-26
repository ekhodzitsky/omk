use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;

use crate::runtime::events::{Event, EventBuilder, EventKind, EventWriter, RunId};
use crate::runtime::scheduler::claim::ClaimStore;
use crate::runtime::scheduler::manifest::RunManifest;
use crate::runtime::scheduler::ownership::OwnershipMap;
use crate::runtime::scheduler::pool::{PoolAction, PoolManager};
use crate::runtime::scheduler::runner::{RunSummary, TeamRunner};
use crate::runtime::scheduler::task::{Task, TaskState};
use crate::runtime::worker::WorkerSpec;
use chrono::Utc;
use tokio_util::sync::CancellationToken;

const STALE_WORKER_CLEANUP_FILE: &str = "stale-worker-cleanup.json";

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

        let config = crate::runtime::config::load_config()
            .await
            .unwrap_or_default();
        let pool_manager = PoolManager::new(config.pools);

        Ok(Self {
            manifest,
            claim_store: ClaimStore::new(),
            ownership: OwnershipMap::new(),
            event_writer,
            state_dir: state_dir.to_path_buf(),
            run_id: RunId(run_id.to_string()),
            last_outbox_offsets: HashMap::new(),
            last_heartbeat_ts: HashMap::new(),
            stale_task_owners: HashMap::new(),
            dead_workers: Default::default(),
            pool_manager,
            pending_pool_actions: Vec::new(),
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

        let config = crate::runtime::config::load_config()
            .await
            .unwrap_or_default();
        let pool_manager = PoolManager::new(config.pools);

        Ok(Self {
            manifest,
            claim_store,
            ownership: OwnershipMap::new(),
            event_writer,
            state_dir: state_dir.to_path_buf(),
            run_id: RunId(run_id.to_string()),
            last_outbox_offsets: HashMap::new(),
            last_heartbeat_ts: HashMap::new(),
            stale_task_owners: HashMap::new(),
            dead_workers: Default::default(),
            pool_manager,
            pending_pool_actions: Vec::new(),
        })
    }

    pub(crate) fn set_lease_seconds(&mut self, secs: i64) {
        self.claim_store.set_lease_seconds(secs);
    }

    /// Drain any pending pool release actions and promote queued tasks.
    pub(crate) async fn drain_pool_actions(&mut self) -> Result<()> {
        for action in self.pending_pool_actions.drain(..) {
            match action {
                PoolAction::Release {
                    pool,
                    task_id,
                    disk_delta,
                } => {
                    if disk_delta != 0 {
                        self.pool_manager.update_disk_usage(&pool, disk_delta).await;
                    }
                    if let Some(promoted) = self.pool_manager.release_slot(&pool, &task_id).await? {
                        // The promoted task remains Pending in the claim store,
                        // so the next dispatch loop will see it as claimable.
                        tracing::info!(
                            pool = %pool,
                            promoted_task = %promoted.task_id,
                            "Queued task promoted to pending after slot release"
                        );
                    }
                }
            }
        }
        Ok(())
    }

    /// Run the main loop until all tasks are done.
    pub async fn run(&mut self, worker_specs: &[WorkerSpec]) -> Result<RunSummary> {
        let cancel = CancellationToken::new();
        self.run_with_cancel(worker_specs, &cancel).await
    }

    pub(crate) async fn run_with_cancel(
        &mut self,
        worker_specs: &[WorkerSpec],
        cancel: &CancellationToken,
    ) -> Result<RunSummary> {
        self.run_with_cancel_reason(worker_specs, cancel, "controller interrupt")
            .await
    }

    pub(crate) async fn run_with_cancel_reason(
        &mut self,
        worker_specs: &[WorkerSpec],
        cancel: &CancellationToken,
        cancel_reason: &str,
    ) -> Result<RunSummary> {
        loop {
            if cancel.is_cancelled() {
                self.cancel_unfinished_tasks(cancel_reason).await?;
                self.drain_pool_actions().await?;
                self.snapshot().await?;
                break;
            }

            self.dispatch_to_workers(worker_specs).await?;
            self.drain_pool_actions().await?;
            self.poll_workers().await?;

            if cancel.is_cancelled() {
                self.cancel_unfinished_tasks(cancel_reason).await?;
                self.drain_pool_actions().await?;
                self.snapshot().await?;
                break;
            }

            self.recover_stale_leases().await?;
            self.drain_pool_actions().await?;
            self.fail_unfinished_tasks_if_no_live_workers(worker_specs)
                .await?;

            self.snapshot().await?;

            if self.claim_store.all_done() {
                break;
            }

            tokio::select! {
                biased;
                _ = cancel.cancelled() => {}
                _ = tokio::time::sleep(std::time::Duration::from_secs(
                    crate::runtime::scheduler::runner::RUNNER_POLL_INTERVAL_SECS,
                )) => {}
            }
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

    async fn cancel_unfinished_tasks(&mut self, reason: &str) -> Result<()> {
        let task_ids: Vec<String> = self
            .claim_store
            .tasks()
            .values()
            .filter(|task| !task.state.is_terminal())
            .map(|task| task.id.clone())
            .collect();

        for task_id in task_ids {
            let worker_id = self
                .claim_store
                .get(&task_id)
                .and_then(|task| task.owner.clone());
            if self.claim_store.cancel(&task_id)? {
                if let Some(task) = self.claim_store.get(&task_id) {
                    self.ownership.release_task(task);
                    self.pending_pool_actions.push(
                        crate::runtime::scheduler::pool::PoolAction::Release {
                            pool: task.pool.clone(),
                            task_id: task_id.clone(),
                            disk_delta: 0,
                        },
                    );
                }
                let event = Event::new(self.run_id.clone(), EventKind::TaskFailed)
                    .with_actor("scheduler")
                    .with_payload(serde_json::json!({
                        "task_id": task_id,
                        "worker_id": worker_id,
                        "error": reason,
                    }))?;
                self.event_writer.append(&event).await?;
            }
        }

        Ok(())
    }

    /// Save current state to manifest.
    pub async fn snapshot(&self) -> Result<()> {
        let mut manifest = self.manifest.clone();
        manifest.tasks = self.claim_store.tasks().values().cloned().collect();
        manifest.save().await?;
        manifest.snapshot_tasks().await?;
        Ok(())
    }

    pub(crate) async fn recover_stale_leases(&mut self) -> Result<()> {
        let recovered = self.claim_store.recover_stale_leases_with_owners();
        for recovery in &recovered {
            if let Some(task) = self.claim_store.get(&recovery.task_id) {
                self.ownership.release_task(task);
                self.pending_pool_actions.push(
                    crate::runtime::scheduler::pool::PoolAction::Release {
                        pool: task.pool.clone(),
                        task_id: recovery.task_id.clone(),
                        disk_delta: 0,
                    },
                );
            }
            if let Some(stale_owner) = recovery.stale_owner.as_deref() {
                self.stale_task_owners
                    .insert(recovery.task_id.clone(), stale_owner.to_string());
                self.quarantine_stale_worker(stale_owner, &recovery.task_id)
                    .await?;
            }
            let event = Event::new(self.run_id.clone(), EventKind::RetryScheduled)
                .with_actor("scheduler")
                .with_payload(serde_json::json!({
                    "task_id": recovery.task_id,
                    "reason": "stale lease recovered",
                    "stale_worker_id": recovery.stale_owner,
                }))?;
            self.event_writer.append(&event).await?;
        }

        Ok(())
    }

    async fn quarantine_stale_worker(&mut self, worker_id: &str, task_id: &str) -> Result<()> {
        if !self.dead_workers.insert(worker_id.to_string()) {
            return Ok(());
        }

        let cleaned_at = Utc::now();
        let marker = serde_json::json!({
            "worker_id": worker_id,
            "task_id": task_id,
            "reason": "stale lease recovered",
            "action": "quarantined",
            "cleaned_at": cleaned_at,
        });
        let worker_dir = self
            .state_dir
            .join(crate::runtime::config::WORKERS_DIR)
            .join(worker_id);
        crate::runtime::config::ensure_private_dir(&worker_dir).await?;
        tokio::fs::write(
            worker_dir.join(STALE_WORKER_CLEANUP_FILE),
            serde_json::to_string_pretty(&marker)?,
        )
        .await?;

        let event = Event::new(self.run_id.clone(), EventKind::WorkerDead)
            .with_actor("scheduler")
            .with_payload(serde_json::json!({
                "worker_id": worker_id,
                "task_id": task_id,
                "reason": "stale lease recovered",
                "cleanup_marker": format!("{}/{}/{}", crate::runtime::config::WORKERS_DIR, worker_id, STALE_WORKER_CLEANUP_FILE),
                "action": "quarantined",
            }))?;
        self.event_writer.append(&event).await
    }

    async fn fail_unfinished_tasks_if_no_live_workers(
        &mut self,
        worker_specs: &[WorkerSpec],
    ) -> Result<()> {
        let has_live_worker = worker_specs
            .iter()
            .any(|worker| !self.dead_workers.contains(&worker.name));
        if has_live_worker {
            return Ok(());
        }

        let reason = "no live workers available after stale cleanup";
        let task_ids: Vec<String> = self
            .claim_store
            .tasks()
            .values()
            .filter(|task| !task.state.is_terminal())
            .map(|task| task.id.clone())
            .collect();

        for task_id in task_ids {
            let previous_worker = self
                .claim_store
                .get(&task_id)
                .and_then(|task| task.owner.clone());

            if let Some(task) = self.claim_store.get(&task_id) {
                self.ownership.release_task(task);
                self.pending_pool_actions.push(
                    crate::runtime::scheduler::pool::PoolAction::Release {
                        pool: task.pool.clone(),
                        task_id: task_id.clone(),
                        disk_delta: 0,
                    },
                );
            }

            let Some(task) = self.claim_store.tasks_mut().get_mut(&task_id) else {
                continue;
            };
            task.state = TaskState::Failed;
            task.owner = None;
            task.lease_expires = None;
            task.completed_at = Some(Utc::now());
            task.started_at = None;

            let event = Event::new(self.run_id.clone(), EventKind::TaskFailed)
                .with_actor("scheduler")
                .with_payload(serde_json::json!({
                    "task_id": task_id,
                    "worker_id": previous_worker,
                    "error": reason,
                }))?;
            self.event_writer.append(&event).await?;
        }

        Ok(())
    }
}
