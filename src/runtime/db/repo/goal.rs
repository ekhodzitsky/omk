use rusqlite::params;

use crate::runtime::db::error::DbError;
use crate::runtime::db::types::{GoalFilter, GoalRecord, GoalSummary};

/// Operations on the `goals` table.
#[allow(async_fn_in_trait)]
pub trait GoalRepo {
    async fn create(&self, goal: &GoalRecord) -> Result<(), DbError>;
    async fn upsert(&self, goal: &GoalRecord) -> Result<(), DbError>;
    async fn get(&self, goal_id: &str) -> Result<Option<GoalRecord>, DbError>;
    async fn update_status(&self, goal_id: &str, status: &str, phase: &str) -> Result<(), DbError>;
    async fn update_controller_pid(
        &self,
        goal_id: &str,
        pid: Option<i32>,
    ) -> Result<(), DbError>;
    async fn heartbeat(&self, goal_id: &str) -> Result<(), DbError>;
    async fn list_running(&self) -> Result<Vec<GoalSummary>, DbError>;
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
                        goal_id, status, phase, kind, original_goal, normalized_goal,
                        goal_text, project_dir, state_dir, policy, delivery_policy,
                        merge_policy, until_ready, slice_execution, max_agents,
                        budget_time_secs, budget_tokens, budget_usd, cost_tracker_path,
                        terminal_criteria, failure, created_at, updated_at,
                        completed_at, controller_pid, version
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26)",
                    params![
                        goal.goal_id,
                        goal.status,
                        goal.phase,
                        goal.kind,
                        goal.original_goal,
                        goal.normalized_goal,
                        goal.goal_text,
                        goal.project_dir,
                        goal.state_dir,
                        goal.policy,
                        goal.delivery_policy,
                        goal.merge_policy,
                        if goal.until_ready { 1 } else { 0 },
                        if goal.slice_execution { 1 } else { 0 },
                        goal.max_agents,
                        goal.budget_time_secs,
                        goal.budget_tokens,
                        goal.budget_usd,
                        goal.cost_tracker_path,
                        goal.terminal_criteria,
                        goal.failure,
                        goal.created_at,
                        goal.updated_at,
                        goal.completed_at,
                        goal.controller_pid,
                        goal.version,
                    ],
                )?;
                Ok(())
            })
            .await
            .map_err(DbError::Connection)
    }

    async fn upsert(&self, goal: &GoalRecord) -> Result<(), DbError> {
        let goal = goal.clone();
        self.conn
            .call(move |conn| {
                conn.execute(
                    "INSERT INTO goals (
                        goal_id, status, phase, kind, original_goal, normalized_goal,
                        goal_text, project_dir, state_dir, policy, delivery_policy,
                        merge_policy, until_ready, slice_execution, max_agents,
                        budget_time_secs, budget_tokens, budget_usd, cost_tracker_path,
                        terminal_criteria, failure, created_at, updated_at,
                        completed_at, controller_pid, version
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26)
                    ON CONFLICT(goal_id) DO UPDATE SET
                        status = excluded.status,
                        phase = excluded.phase,
                        kind = excluded.kind,
                        original_goal = excluded.original_goal,
                        normalized_goal = excluded.normalized_goal,
                        goal_text = excluded.goal_text,
                        project_dir = excluded.project_dir,
                        state_dir = excluded.state_dir,
                        policy = excluded.policy,
                        delivery_policy = excluded.delivery_policy,
                        merge_policy = excluded.merge_policy,
                        until_ready = excluded.until_ready,
                        slice_execution = excluded.slice_execution,
                        max_agents = excluded.max_agents,
                        budget_time_secs = excluded.budget_time_secs,
                        budget_tokens = excluded.budget_tokens,
                        budget_usd = excluded.budget_usd,
                        cost_tracker_path = excluded.cost_tracker_path,
                        terminal_criteria = excluded.terminal_criteria,
                        failure = excluded.failure,
                        created_at = excluded.created_at,
                        updated_at = excluded.updated_at,
                        completed_at = excluded.completed_at,
                        controller_pid = excluded.controller_pid,
                        version = excluded.version",
                    params![
                        goal.goal_id,
                        goal.status,
                        goal.phase,
                        goal.kind,
                        goal.original_goal,
                        goal.normalized_goal,
                        goal.goal_text,
                        goal.project_dir,
                        goal.state_dir,
                        goal.policy,
                        goal.delivery_policy,
                        goal.merge_policy,
                        if goal.until_ready { 1 } else { 0 },
                        if goal.slice_execution { 1 } else { 0 },
                        goal.max_agents,
                        goal.budget_time_secs,
                        goal.budget_tokens,
                        goal.budget_usd,
                        goal.cost_tracker_path,
                        goal.terminal_criteria,
                        goal.failure,
                        goal.created_at,
                        goal.updated_at,
                        goal.completed_at,
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
                        goal_id, status, phase, kind, original_goal, normalized_goal,
                        goal_text, project_dir, state_dir, policy, delivery_policy,
                        merge_policy, until_ready, slice_execution, max_agents,
                        budget_time_secs, budget_tokens, budget_usd, cost_tracker_path,
                        terminal_criteria, failure, created_at, updated_at,
                        completed_at, controller_pid, version
                    FROM goals WHERE goal_id = ?1",
                )?;
                let mut rows = stmt.query(params![goal_id])?;
                if let Some(row) = rows.next()? {
                    Ok(Some(GoalRecord {
                        goal_id: row.get(0)?,
                        status: row.get(1)?,
                        phase: row.get(2)?,
                        kind: row.get(3)?,
                        original_goal: row.get(4)?,
                        normalized_goal: row.get(5)?,
                        goal_text: row.get(6)?,
                        project_dir: row.get(7)?,
                        state_dir: row.get(8)?,
                        policy: row.get(9)?,
                        delivery_policy: row.get(10)?,
                        merge_policy: row.get(11)?,
                        until_ready: row.get::<_, i32>(12)? != 0,
                        slice_execution: row.get::<_, i32>(13)? != 0,
                        max_agents: row.get(14)?,
                        budget_time_secs: row.get(15)?,
                        budget_tokens: row.get(16)?,
                        budget_usd: row.get(17)?,
                        cost_tracker_path: row.get(18)?,
                        terminal_criteria: row.get(19)?,
                        failure: row.get(20)?,
                        created_at: row.get(21)?,
                        updated_at: row.get(22)?,
                        completed_at: row.get(23)?,
                        controller_pid: row.get(24)?,
                        version: row.get(25)?,
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

    async fn update_controller_pid(
        &self,
        goal_id: &str,
        pid: Option<i32>,
    ) -> Result<(), DbError> {
        let goal_id = goal_id.to_string();
        let goal_id_for_err = goal_id.clone();
        let count = self
            .conn
            .call(move |conn| {
                conn.execute(
                    "UPDATE goals SET controller_pid = ?1, updated_at = ?2 WHERE goal_id = ?3",
                    params![pid, chrono::Utc::now().timestamp(), goal_id],
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

    async fn heartbeat(&self, goal_id: &str) -> Result<(), DbError> {
        let goal_id = goal_id.to_string();
        let goal_id_for_err = goal_id.clone();
        let count = self
            .conn
            .call(move |conn| {
                conn.execute(
                    "UPDATE goals SET updated_at = ?1 WHERE goal_id = ?2",
                    params![chrono::Utc::now().timestamp(), goal_id],
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

    async fn list_running(&self) -> Result<Vec<GoalSummary>, DbError> {
        self.conn
            .call(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT goal_id, status, phase, goal_text, created_at, updated_at
                     FROM goals
                     WHERE status = 'running'
                     ORDER BY updated_at DESC",
                )?;
                let rows = stmt.query_map([], |row| {
                    Ok(GoalSummary {
                        goal_id: row.get(0)?,
                        status: row.get(1)?,
                        phase: row.get(2)?,
                        goal_text: row.get(3)?,
                        created_at: row.get(4)?,
                        updated_at: row.get(5)?,
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
