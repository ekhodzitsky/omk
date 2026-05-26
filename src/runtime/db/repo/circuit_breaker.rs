//! Circuit breaker persistence layer.

use chrono::{DateTime, Utc};
use rusqlite::params;

use crate::runtime::db::error::DbError;

/// A persisted circuit breaker record.
#[derive(Debug, Clone)]
pub struct CircuitBreakerRecord {
    pub id: String,
    pub gate_name: String,
    pub project_path: String,
    pub state: String,
    pub consecutive_failures: i64,
    pub failure_threshold: i64,
    pub recovery_timeout_secs: i64,
    pub half_open_max_calls: i64,
    pub half_open_calls_remaining: i64,
    pub last_failure_at: Option<DateTime<Utc>>,
    pub last_success_at: Option<DateTime<Utc>>,
    pub opened_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
}

/// Operations on the `circuit_breakers` table.
#[allow(async_fn_in_trait)]
pub trait CircuitBreakerRepo {
    async fn load_all(&self) -> Result<Vec<CircuitBreakerRecord>, DbError>;
    async fn save(&self, record: &CircuitBreakerRecord) -> Result<(), DbError>;
    async fn delete(&self, id: &str) -> Result<(), DbError>;
}

#[derive(Debug, Clone)]
pub struct CircuitBreakerRepoImpl {
    pub(crate) conn: tokio_rusqlite::Connection,
}

impl CircuitBreakerRepo for CircuitBreakerRepoImpl {
    async fn load_all(&self) -> Result<Vec<CircuitBreakerRecord>, DbError> {
        self.conn
            .call(|conn| {
                let mut stmt = conn.prepare(
                    "SELECT
                        id, gate_name, project_path, state,
                        consecutive_failures, failure_threshold,
                        recovery_timeout_secs, half_open_max_calls,
                        half_open_calls_remaining,
                        last_failure_at, last_success_at, opened_at, updated_at
                     FROM circuit_breakers",
                )?;
                let rows = stmt.query_map([], |row| {
                    Ok(CircuitBreakerRecord {
                        id: row.get(0)?,
                        gate_name: row.get(1)?,
                        project_path: row.get(2)?,
                        state: row.get(3)?,
                        consecutive_failures: row.get(4)?,
                        failure_threshold: row.get(5)?,
                        recovery_timeout_secs: row.get(6)?,
                        half_open_max_calls: row.get(7)?,
                        half_open_calls_remaining: row.get(8)?,
                        last_failure_at: parse_optional_datetime(row.get(9)?),
                        last_success_at: parse_optional_datetime(row.get(10)?),
                        opened_at: parse_optional_datetime(row.get(11)?),
                        updated_at: parse_datetime(row.get(12)?),
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

    async fn save(&self, record: &CircuitBreakerRecord) -> Result<(), DbError> {
        let record = record.clone();
        self.conn
            .call(move |conn| {
                conn.execute(
                    "INSERT INTO circuit_breakers (
                        id, gate_name, project_path, state,
                        consecutive_failures, failure_threshold,
                        recovery_timeout_secs, half_open_max_calls,
                        half_open_calls_remaining,
                        last_failure_at, last_success_at, opened_at, updated_at
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
                    ON CONFLICT(id) DO UPDATE SET
                        gate_name = excluded.gate_name,
                        project_path = excluded.project_path,
                        state = excluded.state,
                        consecutive_failures = excluded.consecutive_failures,
                        failure_threshold = excluded.failure_threshold,
                        recovery_timeout_secs = excluded.recovery_timeout_secs,
                        half_open_max_calls = excluded.half_open_max_calls,
                        half_open_calls_remaining = excluded.half_open_calls_remaining,
                        last_failure_at = excluded.last_failure_at,
                        last_success_at = excluded.last_success_at,
                        opened_at = excluded.opened_at,
                        updated_at = excluded.updated_at",
                    params![
                        record.id,
                        record.gate_name,
                        record.project_path,
                        record.state,
                        record.consecutive_failures,
                        record.failure_threshold,
                        record.recovery_timeout_secs,
                        record.half_open_max_calls,
                        record.half_open_calls_remaining,
                        record.last_failure_at.map(|dt| dt.to_rfc3339()),
                        record.last_success_at.map(|dt| dt.to_rfc3339()),
                        record.opened_at.map(|dt| dt.to_rfc3339()),
                        record.updated_at.to_rfc3339(),
                    ],
                )?;
                Ok(())
            })
            .await
            .map_err(DbError::Connection)
    }

    async fn delete(&self, id: &str) -> Result<(), DbError> {
        let id = id.to_string();
        self.conn
            .call(move |conn| {
                conn.execute("DELETE FROM circuit_breakers WHERE id = ?1", params![id])?;
                Ok(())
            })
            .await
            .map_err(DbError::Connection)
    }
}

fn parse_datetime(s: String) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(&s)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now())
}

fn parse_optional_datetime(s: Option<String>) -> Option<DateTime<Utc>> {
    s.and_then(|txt| {
        DateTime::parse_from_rfc3339(&txt)
            .map(|dt| dt.with_timezone(&Utc))
            .ok()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::db::handle::DbHandle;

    #[tokio::test]
    async fn test_save_and_load() {
        let tmp = tempfile::tempdir().unwrap();
        let db = DbHandle::open(tmp.path().join("test.db")).await.unwrap();
        let repo = CircuitBreakerRepoImpl { conn: db.conn };

        let record = CircuitBreakerRecord {
            id: "/tmp/proj:test".to_string(),
            gate_name: "test".to_string(),
            project_path: "/tmp/proj".to_string(),
            state: "open".to_string(),
            consecutive_failures: 5,
            failure_threshold: 5,
            recovery_timeout_secs: 30,
            half_open_max_calls: 1,
            half_open_calls_remaining: 0,
            last_failure_at: Some(Utc::now()),
            last_success_at: None,
            opened_at: Some(Utc::now()),
            updated_at: Utc::now(),
        };

        repo.save(&record).await.unwrap();
        let loaded = repo.load_all().await.unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id, record.id);
        assert_eq!(loaded[0].state, "open");
        assert_eq!(loaded[0].consecutive_failures, 5);
    }

    #[tokio::test]
    async fn test_delete() {
        let tmp = tempfile::tempdir().unwrap();
        let db = DbHandle::open(tmp.path().join("test.db")).await.unwrap();
        let repo = CircuitBreakerRepoImpl { conn: db.conn };

        let record = CircuitBreakerRecord {
            id: "/tmp/proj:test".to_string(),
            gate_name: "test".to_string(),
            project_path: "/tmp/proj".to_string(),
            state: "closed".to_string(),
            consecutive_failures: 0,
            failure_threshold: 5,
            recovery_timeout_secs: 30,
            half_open_max_calls: 1,
            half_open_calls_remaining: 0,
            last_failure_at: None,
            last_success_at: None,
            opened_at: None,
            updated_at: Utc::now(),
        };

        repo.save(&record).await.unwrap();
        repo.delete(&record.id).await.unwrap();
        let loaded = repo.load_all().await.unwrap();
        assert!(loaded.is_empty());
    }
}
