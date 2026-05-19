use rusqlite::params;

use crate::runtime::db::error::DbError;
use crate::runtime::db::types::ArtifactRecord;

/// Operations on the `artifacts` table.
#[allow(async_fn_in_trait)]
pub trait ArtifactRepo {
    async fn register(
        &self,
        goal_id: &str,
        kind: &str,
        path: &str,
        mime_type: Option<&str>,
    ) -> Result<(), DbError>;
    async fn get_by_goal(
        &self,
        goal_id: &str,
        kind: Option<&str>,
    ) -> Result<Vec<ArtifactRecord>, DbError>;
    async fn delete_by_goal(&self, goal_id: &str) -> Result<(), DbError>;
}

#[derive(Debug, Clone)]
pub struct ArtifactRepoImpl {
    pub(crate) conn: tokio_rusqlite::Connection,
}

impl ArtifactRepo for ArtifactRepoImpl {
    async fn register(
        &self,
        goal_id: &str,
        kind: &str,
        path: &str,
        mime_type: Option<&str>,
    ) -> Result<(), DbError> {
        let goal_id = goal_id.to_string();
        let kind = kind.to_string();
        let path = path.to_string();
        let mime_type = mime_type.map(String::from);
        let created_at = chrono::Utc::now().timestamp();
        self.conn
            .call(move |conn| {
                conn.execute(
                    "INSERT INTO artifacts (goal_id, kind, path, mime_type, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![goal_id, kind, path, mime_type.as_deref(), created_at],
                )?;
                Ok(())
            })
            .await
            .map_err(DbError::Connection)
    }

    async fn get_by_goal(
        &self,
        goal_id: &str,
        kind: Option<&str>,
    ) -> Result<Vec<ArtifactRecord>, DbError> {
        let goal_id = goal_id.to_string();
        let kind = kind.map(String::from);
        self.conn
            .call(move |conn| {
                let (sql, params_vec): (String, Vec<&dyn rusqlite::ToSql>) = if let Some(ref k) = kind {
                    (
                        "SELECT artifact_id, goal_id, kind, path, mime_type, created_at FROM artifacts WHERE goal_id = ?1 AND kind = ?2 ORDER BY created_at".to_string(),
                        vec![&goal_id, k],
                    )
                } else {
                    (
                        "SELECT artifact_id, goal_id, kind, path, mime_type, created_at FROM artifacts WHERE goal_id = ?1 ORDER BY created_at".to_string(),
                        vec![&goal_id],
                    )
                };
                let mut stmt = conn.prepare(&sql)?;
                let rows = stmt.query_map(&*params_vec, |row| {
                    Ok(ArtifactRecord {
                        artifact_id: row.get(0)?,
                        goal_id: row.get(1)?,
                        kind: row.get(2)?,
                        path: row.get(3)?,
                        mime_type: row.get(4)?,
                        created_at: row.get(5)?,
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
                conn.execute("DELETE FROM artifacts WHERE goal_id = ?1", params![goal_id])?;
                Ok(())
            })
            .await
            .map_err(DbError::Connection)
    }
}
