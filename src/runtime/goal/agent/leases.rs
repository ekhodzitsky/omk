use std::sync::Arc;
use std::time::Duration;

use tokio_util::sync::CancellationToken;
use tracing::{debug, warn};

use crate::runtime::db::{DbHandle, SliceLease, SliceLeaseRepo};

const DEFAULT_HEARTBEAT_INTERVAL_SECS: u64 = 10;
const DEFAULT_STALE_THRESHOLD_SECS: u64 = 30;

#[derive(Debug, thiserror::Error)]
pub enum LeaseError {
    #[error(
        "slice already claimed: lease_id={lease_id} by pid={pid} role={role} slice_id={slice_id}"
    )]
    Conflict {
        lease_id: String,
        pid: u32,
        role: String,
        slice_id: String,
    },
    #[error("write-set overlap with active lease: lease_id={lease_id} overlap={overlap:?}")]
    WriteSetOverlap {
        lease_id: String,
        overlap: Vec<String>,
    },
    #[error("database error: {0}")]
    Db(#[from] anyhow::Error),
}

#[derive(Debug)]
pub struct LeaseManager {
    db: DbHandle,
    pub owner_pid: u32,
    stale_threshold: Duration,
    heartbeat_interval: Duration,
}

#[derive(Debug)]
pub struct LeaseGuard {
    pub lease_id: String,
    manager: Arc<LeaseManager>,
    cancel: CancellationToken,
    heartbeat_handle: Option<tokio::task::JoinHandle<()>>,
    released: bool,
}

impl LeaseManager {
    pub fn new(db: DbHandle) -> Self {
        Self {
            db,
            owner_pid: std::process::id(),
            stale_threshold: Duration::from_secs(DEFAULT_STALE_THRESHOLD_SECS),
            heartbeat_interval: Duration::from_secs(DEFAULT_HEARTBEAT_INTERVAL_SECS),
        }
    }

    /// Attempt to claim a lease for a slice.
    ///
    /// Before claiming, stale leases are expired. On success returns a guard
    /// that auto-releases on drop and spawns a heartbeat task.
    pub async fn try_claim(
        self: &Arc<Self>,
        goal_id: &str,
        slice_id: &str,
        owner_role: &str,
        write_set: Vec<String>,
    ) -> Result<(LeaseGuard, Vec<SliceLease>), LeaseError> {
        let now_unix = chrono::Utc::now().timestamp();

        // Expire stale leases first.
        let expired_leases = self
            .db
            .slice_lease_repo()
            .expire_stale(now_unix, self.stale_threshold.as_secs() as i64)
            .await
            .map_err(|e| LeaseError::Db(anyhow::anyhow!("expire_stale failed: {e}")))?;

        let lease = self
            .db
            .slice_lease_repo()
            .try_claim(
                goal_id,
                slice_id,
                self.owner_pid,
                owner_role,
                &write_set,
                now_unix,
            )
            .await
            .map_err(|e| LeaseError::Db(anyhow::anyhow!("{e}")))?;

        match lease {
            Some(lease) => {
                let cancel = CancellationToken::new();
                let heartbeat_cancel = cancel.clone();
                let manager = Arc::clone(self);
                let lease_id = lease.lease_id.clone();

                let handle = tokio::spawn(async move {
                    let mut interval = tokio::time::interval(manager.heartbeat_interval);
                    loop {
                        tokio::select! {
                            _ = interval.tick() => {
                                let now = chrono::Utc::now().timestamp();
                                if let Err(e) = manager
                                    .db
                                    .slice_lease_repo()
                                    .heartbeat(&lease_id, now)
                                    .await
                                {
                                    warn!(lease_id, error = %e, "Slice lease heartbeat failed");
                                }
                            }
                            _ = heartbeat_cancel.cancelled() => break,
                        }
                    }
                    debug!(lease_id, "Slice lease heartbeat loop stopped");
                });

                Ok((
                    LeaseGuard {
                        lease_id: lease.lease_id,
                        manager: Arc::clone(self),
                        cancel,
                        heartbeat_handle: Some(handle),
                        released: false,
                    },
                    expired_leases,
                ))
            }
            None => {
                // Determine whether this is a slice conflict or write-set overlap
                // by inspecting active leases.
                let active = self
                    .db
                    .slice_lease_repo()
                    .active_for_goal(goal_id)
                    .await
                    .map_err(|e| LeaseError::Db(anyhow::anyhow!("{e}")))?;

                if let Some(existing) = active.iter().find(|l| l.slice_id == slice_id) {
                    return Err(LeaseError::Conflict {
                        lease_id: existing.lease_id.clone(),
                        pid: existing.owner_pid,
                        role: existing.owner_role.clone(),
                        slice_id: slice_id.to_string(),
                    });
                }

                for existing in &active {
                    let overlap: Vec<String> = write_set
                        .iter()
                        .filter(|path| {
                            existing
                                .write_set
                                .iter()
                                .any(|other| paths_conflict(path, other))
                        })
                        .cloned()
                        .collect();
                    if !overlap.is_empty() {
                        return Err(LeaseError::WriteSetOverlap {
                            lease_id: existing.lease_id.clone(),
                            overlap,
                        });
                    }
                }

                // Race: the conflicting lease may have been released between
                // try_claim and active_for_goal. Return a generic conflict.
                Err(LeaseError::Conflict {
                    lease_id: "unknown".to_string(),
                    pid: 0,
                    role: "unknown".to_string(),
                    slice_id: slice_id.to_string(),
                })
            }
        }
    }
}

impl LeaseGuard {
    pub fn lease_id(&self) -> &str {
        &self.lease_id
    }

    /// Explicitly release the lease (consumes the guard).
    pub async fn release(mut self) {
        self.cancel.cancel();
        if let Some(handle) = self.heartbeat_handle.take() {
            let _ = handle.await;
        }
        let now = chrono::Utc::now().timestamp();
        if let Err(e) = self
            .manager
            .db
            .slice_lease_repo()
            .release(&self.lease_id, now)
            .await
        {
            warn!(lease_id = %self.lease_id, error = %e, "Failed to release slice lease");
        }
        self.released = true;
    }
}

impl Drop for LeaseGuard {
    fn drop(&mut self) {
        if self.released {
            return;
        }
        self.cancel.cancel();
        if let Some(handle) = self.heartbeat_handle.take() {
            handle.abort();
        }
        let lease_id = self.lease_id.clone();
        let db = self.manager.db.clone();
        tokio::spawn(async move {
            let now = chrono::Utc::now().timestamp();
            if let Err(e) = db.slice_lease_repo().release(&lease_id, now).await {
                warn!(lease_id, error = %e, "Best-effort slice lease release on drop failed");
            }
        });
    }
}

fn paths_conflict(a: &str, b: &str) -> bool {
    let a = a.trim();
    let b = b.trim();
    if a == "project files" || b == "project files" {
        return true;
    }
    a == b || a.starts_with(&format!("{b}/")) || b.starts_with(&format!("{a}/"))
}
