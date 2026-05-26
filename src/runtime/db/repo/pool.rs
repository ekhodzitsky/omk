use chrono::{TimeZone, Utc};
use rusqlite::params;

use crate::runtime::scheduler::pool::QueuedTask;
use crate::runtime::scheduler::pool_repo::{PoolQueueRecord, PoolRepo, PoolRepoError};

#[derive(Debug, Clone)]
pub struct PoolRepoImpl {
    pub(crate) conn: tokio_rusqlite::Connection,
}

impl PoolRepo for PoolRepoImpl {
    async fn save_queue(
        &self,
        pool_name: &str,
        run_id: &str,
        queue: &[QueuedTask],
    ) -> Result<(), PoolRepoError> {
        let pool_name = pool_name.to_string();
        let run_id = run_id.to_string();
        let queue = queue.to_vec();
        self.conn
            .call(move |conn| -> Result<(), rusqlite::Error> {
                conn.execute(
                    "DELETE FROM pool_queue WHERE run_id = ?1 AND pool_name = ?2",
                    params![&run_id, &pool_name],
                )?;
                for item in &queue {
                    let enqueued_at = item.enqueued_at.timestamp();
                    conn.execute(
                        "INSERT INTO pool_queue (pool_name, task_id, priority, enqueued_at, run_id)
                         VALUES (?1, ?2, ?3, ?4, ?5)",
                        params![
                            &pool_name,
                            &item.task_id,
                            item.priority,
                            enqueued_at,
                            &run_id,
                        ],
                    )?;
                }
                Ok(())
            })
            .await
            .map_err(|e| PoolRepoError::Database(e.to_string()))?;
        Ok(())
    }

    async fn load_queue(&self, run_id: &str) -> Result<Vec<PoolQueueRecord>, PoolRepoError> {
        let run_id = run_id.to_string();
        self.conn
            .call(
                move |conn| -> Result<Vec<PoolQueueRecord>, rusqlite::Error> {
                    let mut stmt = conn.prepare(
                        "SELECT pool_name, task_id, priority, enqueued_at, run_id
                     FROM pool_queue
                     WHERE run_id = ?1
                     ORDER BY enqueued_at ASC",
                    )?;
                    let rows = stmt.query_map(params![&run_id], |row| {
                        let enqueued_at: i64 = row.get(3)?;
                        let enqueued_dt = Utc
                            .timestamp_opt(enqueued_at, 0)
                            .single()
                            .unwrap_or_else(Utc::now);
                        Ok(PoolQueueRecord {
                            pool_name: row.get(0)?,
                            task_id: row.get(1)?,
                            priority: row.get(2)?,
                            enqueued_at: enqueued_dt,
                            run_id: row.get(4)?,
                        })
                    })?;
                    let mut results = Vec::new();
                    for row in rows {
                        results.push(row?);
                    }
                    Ok(results)
                },
            )
            .await
            .map_err(|e| PoolRepoError::Database(e.to_string()))
    }

    async fn delete_queue(&self, run_id: &str) -> Result<(), PoolRepoError> {
        let run_id = run_id.to_string();
        self.conn
            .call(move |conn| -> Result<(), rusqlite::Error> {
                conn.execute("DELETE FROM pool_queue WHERE run_id = ?1", params![&run_id])?;
                Ok(())
            })
            .await
            .map_err(|e| PoolRepoError::Database(e.to_string()))?;
        Ok(())
    }

    async fn delete_pool_queue(&self, run_id: &str, pool_name: &str) -> Result<(), PoolRepoError> {
        let run_id = run_id.to_string();
        let pool_name = pool_name.to_string();
        self.conn
            .call(move |conn| -> Result<(), rusqlite::Error> {
                conn.execute(
                    "DELETE FROM pool_queue WHERE run_id = ?1 AND pool_name = ?2",
                    params![&run_id, &pool_name],
                )?;
                Ok(())
            })
            .await
            .map_err(|e| PoolRepoError::Database(e.to_string()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::db::handle::DbHandle;

    #[tokio::test]
    async fn pool_repo_roundtrip() {
        let db = DbHandle::open(":memory:").await.unwrap();
        let repo = PoolRepoImpl { conn: db.conn };

        let queue = vec![
            QueuedTask {
                task_id: "t1".to_string(),
                priority: 0,
                enqueued_at: Utc::now(),
            },
            QueuedTask {
                task_id: "t2".to_string(),
                priority: 5,
                enqueued_at: Utc::now() + chrono::Duration::seconds(1),
            },
        ];

        repo.save_queue("default", "run-1", &queue).await.unwrap();
        let loaded = repo.load_queue("run-1").await.unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].task_id, "t1");
        assert_eq!(loaded[1].task_id, "t2");

        repo.delete_queue("run-1").await.unwrap();
        let loaded = repo.load_queue("run-1").await.unwrap();
        assert!(loaded.is_empty());
    }
}
