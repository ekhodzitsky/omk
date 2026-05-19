use rusqlite::params;

use crate::runtime::db::error::DbError;
use crate::runtime::db::types::{GoalFilter, GoalRecord, GoalSummary};

/// Operations on the `goals` table.
#[allow(async_fn_in_trait)]
pub trait GoalRepo {
    async fn create(&self, goal: &GoalRecord) -> Result<(), DbError>;
    async fn get(&self, goal_id: &str) -> Result<Option<GoalRecord>, DbError>;
    async fn update_status(&self, goal_id: &str, status: &str, phase: &str) -> Result<(), DbError>;
    async fn list(&self, filter: GoalFilter) -> Result<Vec<GoalSummary>, DbError>;
    async fn delete(&self, goal_id: &str) -> Result<(), DbError>;
}

#[derive(Debug, Clone)]
pub struct GoalRepoImpl {
    pub(crate) conn: tokio_rusqlite::Connection,
}

impl GoalRepo for GoalRepoImpl {
    async fn create(&self, goal: &GoalRecord) -> Result<(), DbError> {
        let goal = goal.clone();
        self.conn
            .call(move |conn| {
                conn.execute(
                    "INSERT INTO goals (
                        goal_id, status, phase, kind, goal_text, project_dir,
                        policy, merge_policy, slice_execution, max_agents,
                        budget_time_secs, budget_tokens, budget_usd,
                        created_at, updated_at, controller_pid, version
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
                    params![
                        goal.goal_id,
                        goal.status,
                        goal.phase,
                        goal.kind,
                        goal.goal_text,
                        goal.project_dir,
                        goal.policy,
                        goal.merge_policy,
                        if goal.slice_execution { 1 } else { 0 },
                        goal.max_agents,
                        goal.budget_time_secs,
                        goal.budget_tokens,
                        goal.budget_usd,
                        goal.created_at,
                        goal.updated_at,
                        goal.controller_pid,
                        goal.version,
                    ],
                )?;
                Ok(())
            })
            .await
            .map_err(DbError::Connection)
    }

    async fn get(&self, goal_id: &str) -> Result<Option<GoalRecord>, DbError> {
        let goal_id = goal_id.to_string();
        self.conn
            .call(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT
                        goal_id, status, phase, kind, goal_text, project_dir,
                        policy, merge_policy, slice_execution, max_agents,
                        budget_time_secs, budget_tokens, budget_usd,
                        created_at, updated_at, controller_pid, version
                    FROM goals WHERE goal_id = ?1",
                )?;
                let mut rows = stmt.query(params![goal_id])?;
                if let Some(row) = rows.next()? {
                    Ok(Some(GoalRecord {
                        goal_id: row.get(0)?,
                        status: row.get(1)?,
                        phase: row.get(2)?,
                        kind: row.get(3)?,
                        goal_text: row.get(4)?,
                        project_dir: row.get(5)?,
                        policy: row.get(6)?,
                        merge_policy: row.get(7)?,
                        slice_execution: row.get::<_, i32>(8)? != 0,
                        max_agents: row.get(9)?,
                        budget_time_secs: row.get(10)?,
                        budget_tokens: row.get(11)?,
                        budget_usd: row.get(12)?,
                        created_at: row.get(13)?,
                        updated_at: row.get(14)?,
                        controller_pid: row.get(15)?,
                        version: row.get(16)?,
                    }))
                } else {
                    Ok(None)
                }
            })
            .await
            .map_err(DbError::Connection)
    }

    async fn update_status(&self, goal_id: &str, status: &str, phase: &str) -> Result<(), DbError> {
        let goal_id = goal_id.to_string();
        let status = status.to_string();
        let phase = phase.to_string();
        let updated_at = chrono::Utc::now().timestamp();
        let goal_id_for_err = goal_id.clone();
        let count = self
            .conn
            .call(move |conn| {
                conn.execute(
                    "UPDATE goals SET status = ?1, phase = ?2, updated_at = ?3 WHERE goal_id = ?4",
                    params![status, phase, updated_at, goal_id],
                )
                .map_err(tokio_rusqlite::Error::Rusqlite)
            })
            .await
            .map_err(DbError::Connection)?;
        if count == 0 {
            return Err(DbError::GoalNotFound(goal_id_for_err));
        }
        Ok(())
    }

    async fn list(&self, filter: GoalFilter) -> Result<Vec<GoalSummary>, DbError> {
        self.conn
            .call(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT goal_id, status, phase, goal_text, created_at, updated_at
                     FROM goals
                     WHERE (?1 IS NULL OR status = ?1)
                       AND (?2 IS NULL OR phase = ?2)
                       AND (?3 IS NULL OR kind = ?3)
                       AND (?4 IS NULL OR updated_at < ?4)
                     ORDER BY updated_at DESC
                     LIMIT COALESCE(?5, -1)",
                )?;
                let limit = filter.limit.map(|l| l as i64);
                let rows = stmt.query_map(
                    params![
                        filter.status,
                        filter.phase,
                        filter.kind,
                        filter.older_than,
                        limit,
                    ],
                    |row| {
                        Ok(GoalSummary {
                            goal_id: row.get(0)?,
                            status: row.get(1)?,
                            phase: row.get(2)?,
                            goal_text: row.get(3)?,
                            created_at: row.get(4)?,
                            updated_at: row.get(5)?,
                        })
                    },
                )?;

                let mut results = Vec::new();
                for row in rows {
                    results.push(row?);
                }
                Ok(results)
            })
            .await
            .map_err(DbError::Connection)
    }

    async fn delete(&self, goal_id: &str) -> Result<(), DbError> {
        let goal_id = goal_id.to_string();
        let goal_id_for_err = goal_id.clone();
        let count = self
            .conn
            .call(move |conn| {
                conn.execute("DELETE FROM goals WHERE goal_id = ?1", params![goal_id])
                    .map_err(tokio_rusqlite::Error::Rusqlite)
            })
            .await
            .map_err(DbError::Connection)?;
        if count == 0 {
            return Err(DbError::GoalNotFound(goal_id_for_err));
        }
        Ok(())
    }
}
