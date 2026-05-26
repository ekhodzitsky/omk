use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use chrono::{DateTime, Utc};

use crate::runtime::events::{EventWriter, RunId};
use crate::runtime::scheduler::claim::ClaimStore;
use crate::runtime::scheduler::manifest::RunManifest;
use crate::runtime::scheduler::ownership::OwnershipMap;
use crate::runtime::scheduler::pool::{PoolAction, PoolManager};

mod dispatch;
mod poll;
mod run;
mod types;

pub use types::RunSummary;
pub(crate) use types::{ParsedResult, SimpleResult};

/// Poll interval for the runner dispatch loop.
pub const RUNNER_POLL_INTERVAL_SECS: u64 = 1;

/// Orchestrates a team run using the scheduler: claims tasks, dispatches to
/// workers via inbox/outbox, and drives the run to completion.
#[derive(Debug)]
pub struct TeamRunner {
    pub(crate) manifest: RunManifest,
    pub(crate) claim_store: ClaimStore,
    pub(crate) ownership: OwnershipMap,
    event_writer: EventWriter,
    state_dir: PathBuf,
    run_id: RunId,
    last_outbox_offsets: HashMap<String, u64>,
    last_heartbeat_ts: HashMap<String, DateTime<Utc>>,
    stale_task_owners: HashMap<String, String>,
    dead_workers: HashSet<String>,
    pool_manager: PoolManager,
    pending_pool_actions: Vec<PoolAction>,
}

#[cfg(test)]
mod tests;
