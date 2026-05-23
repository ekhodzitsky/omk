use rusqlite::{params, OptionalExtension};
use serde_json;

use crate::runtime::db::error::DbError;

/// A single slice lease record.
#[derive(Debug, Clone)]
pub struct SliceLease {
    pub lease_id: String,
    pub goal_id: String,
    pub slice_id: String,
    pub owner_pid: u32,
    pub owner_role: String,
    pub write_set: Vec<String>,
    pub claimed_at: i64,
    pub heartbeat_at: i64,
    pub released_at: Option<i64>,
    pub expired_at: Option<i64>,
}

/// Operations on the `goal_slice_leases` table.
#[allow(async_fn_in_trait)]
pub trait SliceLeaseRepo {
    /// Attempt to claim a lease for `(goal_id, slice_id)` by `(pid, role)`.
    ///
    /// On success returns the claimed lease. On conflict returns `None`.
    /// Uses a single `BEGIN IMMEDIATE` transaction for atomicity.
    async fn try_claim(
        &self,
        goal_id: &str,
        slice_id: &str,
        owner_pid: u32,
        owner_role: &str,
        write_set: &[String],
        now_unix: i64,
    ) -> Result<Option<SliceLease>, DbError>;

    /// Release a lease by setting `released_at`.
    async fn release(&self, lease_id: &str, now_unix: i64) -> Result<(), DbError>;

    /// Update the heartbeat timestamp for a lease.
    async fn heartbeat(&self, lease_id: &str, now_unix: i64) -> Result<(), DbError>;

    /// Mark stale leases as expired.
    ///
    /// Returns the expired leases. Idempotent.
    async fn expire_stale(
        &self,
        now_unix: i64,
        threshold_secs: i64,
    ) -> Result<Vec<SliceLease>, DbError>;

    /// List all active leases for a goal.
    async fn active_for_goal(&self, goal_id: &str) -> Result<Vec<SliceLease>, DbError>;

    /// Get a lease by id.
    async fn get(&self, lease_id: &str) -> Result<Option<SliceLease>, DbError>;
}

#[derive(Debug, Clone)]
pub struct SliceLeaseRepoImpl {
    pub(crate) conn: tokio_rusqlite::Connection,
}

impl SliceLeaseRepo for SliceLeaseRepoImpl {
    async fn try_claim(
        &self,
        goal_id: &str,
        slice_id: &str,
        owner_pid: u32,
        owner_role: &str,
        write_set: &[String],
        now_unix: i64,
    ) -> Result<Option<SliceLease>, DbError> {
        let goal_id = goal_id.to_string();
        let slice_id = slice_id.to_string();
        let owner_role = owner_role.to_string();
        let write_set_json = serde_json::to_string(write_set).unwrap_or_else(|_| "[]".to_string());
        let write_set_for_check: Vec<String> = write_set.to_vec();
        let lease_id = format!("{}-{}-{}", goal_id, slice_id, uuid::Uuid::new_v4());

        self.conn
            .call(move |conn| -> Result<Option<SliceLease>, rusqlite::Error> {
                if let Err(e) = conn.execute("BEGIN IMMEDIATE", []) {
                    if let rusqlite::Error::SqliteFailure(code, _) = &e {
                        if code.code == rusqlite::ErrorCode::DatabaseBusy {
                            return Ok(None);
                        }
                    }
                    return Err(e);
                }

                let result = (|| -> Result<Option<SliceLease>, rusqlite::Error> {
                    // 1. Check for active lease on the exact slice.
                    let existing_slice: Option<String> = conn
                        .query_row(
                            "SELECT lease_id FROM goal_slice_leases
                             WHERE goal_id = ?1 AND slice_id = ?2
                               AND released_at IS NULL AND expired_at IS NULL",
                            params![&goal_id, &slice_id],
                            |row| row.get(0),
                        )
                        .optional()?;

                    if existing_slice.is_some() {
                        return Ok(None);
                    }

                    // 2. Check for write-set overlap with any active lease in this goal.
                    let mut stmt = conn.prepare(
                        "SELECT lease_id, write_set FROM goal_slice_leases
                         WHERE goal_id = ?1
                           AND released_at IS NULL AND expired_at IS NULL",
                    )?;
                    let rows = stmt.query_map(params![&goal_id], |row| {
                        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                    })?;

                    for row in rows {
                        let (_other_lease_id, other_write_set_json) = row?;
                        let other_write_set: Vec<String> =
                            serde_json::from_str(&other_write_set_json).unwrap_or_default();
                        if write_sets_overlap(&write_set_for_check, &other_write_set) {
                            return Ok(None);
                        }
                    }

                    // 3. Insert the new lease.
                    conn.execute(
                        "INSERT INTO goal_slice_leases
                         (lease_id, goal_id, slice_id, owner_pid, owner_role,
                          claimed_at, heartbeat_at, released_at, expired_at, write_set)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, NULL, ?8)",
                        params![
                            &lease_id,
                            &goal_id,
                            &slice_id,
                            owner_pid,
                            &owner_role,
                            now_unix,
                            now_unix,
                            &write_set_json,
                        ],
                    )?;

                    Ok(Some(SliceLease {
                        lease_id: lease_id.clone(),
                        goal_id: goal_id.clone(),
                        slice_id: slice_id.clone(),
                        owner_pid,
                        owner_role: owner_role.clone(),
                        write_set: write_set_for_check.clone(),
                        claimed_at: now_unix,
                        heartbeat_at: now_unix,
                        released_at: None,
                        expired_at: None,
                    }))
                })();

                match result {
                    Ok(Some(lease)) => {
                        conn.execute("COMMIT", [])?;
                        Ok(Some(lease))
                    }
                    Ok(None) => {
                        let _ = conn.execute("ROLLBACK", []);
                        Ok(None)
                    }
                    Err(e) => {
                        let _ = conn.execute("ROLLBACK", []);
                        Err(e)
                    }
                }
            })
            .await
            .map_err(DbError::Connection)
    }

    async fn release(&self, lease_id: &str, now_unix: i64) -> Result<(), DbError> {
        let lease_id = lease_id.to_string();
        self.conn
            .call(move |conn| -> Result<(), rusqlite::Error> {
                conn.execute(
                    "UPDATE goal_slice_leases SET released_at = ?1 WHERE lease_id = ?2",
                    params![now_unix, lease_id],
                )?;
                Ok(())
            })
            .await
            .map_err(DbError::Connection)
    }

    async fn heartbeat(&self, lease_id: &str, now_unix: i64) -> Result<(), DbError> {
        let lease_id = lease_id.to_string();
        self.conn
            .call(move |conn| -> Result<(), rusqlite::Error> {
                conn.execute(
                    "UPDATE goal_slice_leases SET heartbeat_at = ?1 WHERE lease_id = ?2",
                    params![now_unix, lease_id],
                )?;
                Ok(())
            })
            .await
            .map_err(DbError::Connection)
    }

    async fn expire_stale(
        &self,
        now_unix: i64,
        threshold_secs: i64,
    ) -> Result<Vec<SliceLease>, DbError> {
        let threshold = now_unix - threshold_secs;
        self.conn
            .call(move |conn| -> Result<Vec<SliceLease>, rusqlite::Error> {
                let mut stmt = conn.prepare(
                    "UPDATE goal_slice_leases
                     SET expired_at = ?1
                     WHERE heartbeat_at < ?2
                       AND released_at IS NULL
                       AND expired_at IS NULL
                     RETURNING lease_id, goal_id, slice_id, owner_pid, owner_role,
                               claimed_at, heartbeat_at, released_at, expired_at, write_set",
                )?;
                let rows = stmt.query_map(params![now_unix, threshold], |row| {
                    let write_set_json: String = row.get(9)?;
                    let write_set: Vec<String> =
                        serde_json::from_str(&write_set_json).unwrap_or_default();
                    Ok(SliceLease {
                        lease_id: row.get(0)?,
                        goal_id: row.get(1)?,
                        slice_id: row.get(2)?,
                        owner_pid: row.get::<_, i64>(3)? as u32,
                        owner_role: row.get(4)?,
                        write_set,
                        claimed_at: row.get(5)?,
                        heartbeat_at: row.get(6)?,
                        released_at: row.get(7)?,
                        expired_at: row.get(8)?,
                    })
                })?;
                let mut results = Vec::new();
                for row in rows {
                    results.push(row?);
                }
                Ok(results)
            })
            .await
            .map_err(DbError::Connection)
    }

    async fn active_for_goal(&self, goal_id: &str) -> Result<Vec<SliceLease>, DbError> {
        let goal_id = goal_id.to_string();
        self.conn
            .call(move |conn| -> Result<Vec<SliceLease>, rusqlite::Error> {
                let mut stmt = conn.prepare(
                    "SELECT lease_id, goal_id, slice_id, owner_pid, owner_role,
                            claimed_at, heartbeat_at, released_at, expired_at, write_set
                     FROM goal_slice_leases
                     WHERE goal_id = ?1
                       AND released_at IS NULL
                       AND expired_at IS NULL",
                )?;
                let rows = stmt.query_map(params![&goal_id], |row| {
                    let write_set_json: String = row.get(9)?;
                    let write_set: Vec<String> =
                        serde_json::from_str(&write_set_json).unwrap_or_default();
                    Ok(SliceLease {
                        lease_id: row.get(0)?,
                        goal_id: row.get(1)?,
                        slice_id: row.get(2)?,
                        owner_pid: row.get::<_, i64>(3)? as u32,
                        owner_role: row.get(4)?,
                        write_set,
                        claimed_at: row.get(5)?,
                        heartbeat_at: row.get(6)?,
                        released_at: row.get(7)?,
                        expired_at: row.get(8)?,
                    })
                })?;
                let mut results = Vec::new();
                for row in rows {
                    results.push(row?);
                }
                Ok(results)
            })
            .await
            .map_err(DbError::Connection)
    }

    async fn get(&self, lease_id: &str) -> Result<Option<SliceLease>, DbError> {
        let lease_id = lease_id.to_string();
        self.conn
            .call(move |conn| -> Result<Option<SliceLease>, rusqlite::Error> {
                let mut stmt = conn.prepare(
                    "SELECT lease_id, goal_id, slice_id, owner_pid, owner_role,
                            claimed_at, heartbeat_at, released_at, expired_at, write_set
                     FROM goal_slice_leases
                     WHERE lease_id = ?1",
                )?;
                let mut rows = stmt.query(params![&lease_id])?;
                if let Some(row) = rows.next()? {
                    let write_set_json: String = row.get(9)?;
                    let write_set: Vec<String> =
                        serde_json::from_str(&write_set_json).unwrap_or_default();
                    Ok(Some(SliceLease {
                        lease_id: row.get(0)?,
                        goal_id: row.get(1)?,
                        slice_id: row.get(2)?,
                        owner_pid: row.get::<_, i64>(3)? as u32,
                        owner_role: row.get(4)?,
                        write_set,
                        claimed_at: row.get(5)?,
                        heartbeat_at: row.get(6)?,
                        released_at: row.get(7)?,
                        expired_at: row.get(8)?,
                    }))
                } else {
                    Ok(None)
                }
            })
            .await
            .map_err(DbError::Connection)
    }
}

fn write_sets_overlap(a: &[String], b: &[String]) -> bool {
    a.iter()
        .any(|path| b.iter().any(|other| paths_conflict(path, other)))
}

fn paths_conflict(a: &str, b: &str) -> bool {
    let a = a.trim();
    let b = b.trim();
    if a == "project files" || b == "project files" {
        return true;
    }
    a == b || a.starts_with(&format!("{b}/")) || b.starts_with(&format!("{a}/"))
}
