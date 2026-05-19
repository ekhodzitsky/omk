use rusqlite::params;

use crate::runtime::db::error::DbError;
use crate::runtime::db::types::EventRecord;

/// Operations on the `events` table.
#[allow(async_fn_in_trait)]
pub trait EventRepo {
    async fn append(&self, goal_id: &str, kind: &str, payload: &str) -> Result<i64, DbError>;
    async fn get_by_goal(
        &self,
        goal_id: &str,
        since: Option<i64>,
        limit: Option<usize>,
    ) -> Result<Vec<EventRecord>, DbError>;
    async fn delete_by_goal(&self, goal_id: &str) -> Result<(), DbError>;
}

#[derive(Debug, Clone)]
pub struct EventRepoImpl {
    pub(crate) conn: tokio_rusqlite::Connection,
}

impl EventRepo for EventRepoImpl {
    async fn append(&self, goal_id: &str, kind: &str, payload: &str) -> Result<i64, DbError> {
        let goal_id = goal_id.to_string();
        let kind = kind.to_string();
        let payload = payload.to_string();
        let created_at = chrono::Utc::now().timestamp();
        self.conn
            .call(move |conn| {
                conn.execute(
                    "INSERT INTO events (goal_id, kind, payload, created_at) VALUES (?1, ?2, ?3, ?4)",
                    params![goal_id, kind, payload, created_at],
                )?;
                Ok(conn.last_insert_rowid())
            })
            .await
            .map_err(DbError::Connection)
    }

    async fn get_by_goal(
        &self,
        goal_id: &str,
        since: Option<i64>,
        limit: Option<usize>,
    ) -> Result<Vec<EventRecord>, DbError> {
        let goal_id = goal_id.to_string();
        let limit_i64 = limit.map(|l| l as i64);
        self.conn
            .call(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT event_id, goal_id, kind, payload, created_at
                     FROM events
                     WHERE goal_id = ?1
                       AND (?2 IS NULL OR created_at >= ?2)
                     ORDER BY created_at ASC
                     LIMIT COALESCE(?3, -1)",
                )?;
                let rows = stmt.query_map(params![goal_id, since, limit_i64], |row| {
                    Ok(EventRecord {
                        event_id: row.get(0)?,
                        goal_id: row.get(1)?,
                        kind: row.get(2)?,
                        payload: row.get(3)?,
                        created_at: row.get(4)?,
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

    async fn delete_by_goal(&self, goal_id: &str) -> Result<(), DbError> {
        let goal_id = goal_id.to_string();
        self.conn
            .call(move |conn| {
                conn.execute("DELETE FROM events WHERE goal_id = ?1", params![goal_id])?;
                Ok(())
            })
            .await
            .map_err(DbError::Connection)
    }
}
