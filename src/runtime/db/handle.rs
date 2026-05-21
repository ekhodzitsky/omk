use std::path::Path;

use tokio_rusqlite::Connection;

use super::error::DbError;
use super::migrations::MigrationRunner;
use super::transaction::DbTransaction;

/// A cloneable handle to a SQLite database.
#[derive(Clone, Debug)]
#[must_use = "DbHandle represents an open database connection"]
pub struct DbHandle {
    pub(super) conn: Connection,
}

impl DbHandle {
    /// Open or create a database at `path`, applying migrations and enabling WAL.
    pub async fn open(path: impl AsRef<Path>) -> Result<Self, DbError> {
        let path = path.as_ref().to_path_buf();
        let conn = Connection::open(path).await.map_err(DbError::Connection)?;

        conn.call(|conn| {
            let current: u32 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
            let runner = MigrationRunner::default();
            runner.run(conn, current).map_err(|e| {
                tokio_rusqlite::Error::Other(Box::new(std::io::Error::other(e.to_string())))
            })?;
            Ok(())
        })
        .await
        .map_err(DbError::Connection)?;

        Ok(Self { conn })
    }

    /// Close the connection gracefully.
    pub async fn close(self) -> Result<(), DbError> {
        self.conn.close().await.map_err(DbError::Connection)
    }

    /// Begin a new transaction.
    pub async fn transaction(&self) -> Result<DbTransaction, DbError> {
        self.conn
            .call(|conn| {
                conn.execute("BEGIN", [])
                    .map_err(tokio_rusqlite::Error::Rusqlite)
            })
            .await
            .map_err(DbError::Connection)?;
        Ok(DbTransaction {
            conn: self.conn.clone(),
            active: true,
        })
    }

    /// Backup the database to `dest` using the SQLite backup API.
    pub async fn backup_to(&self, dest: impl AsRef<Path>) -> Result<(), DbError> {
        let dest = dest.as_ref().to_path_buf();
        self.conn
            .call(move |conn| {
                conn.backup(rusqlite::DatabaseName::Main, &dest, None)
                    .map_err(tokio_rusqlite::Error::Rusqlite)
            })
            .await
            .map_err(DbError::Connection)
    }
}
