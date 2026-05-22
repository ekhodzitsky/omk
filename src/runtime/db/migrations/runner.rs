#[derive(Debug)]
pub struct Migration {
    pub version: u32,
    pub name: &'static str,
    pub sql: &'static str,
}

#[derive(Debug)]
pub struct MigrationRunner {
    migrations: Vec<Migration>,
}

impl MigrationRunner {
    pub fn new() -> Self {
        Self {
            migrations: vec![
                Migration {
                    version: 1,
                    name: "initial",
                    sql: include_str!("001_initial.sql"),
                },
                Migration {
                    version: 2,
                    name: "slice_leases",
                    sql: include_str!("002_slice_leases.sql"),
                },
            ],
        }
    }

    pub fn target_version(&self) -> u32 {
        self.migrations.iter().map(|m| m.version).max().unwrap_or(0)
    }

    /// Apply all migrations with version > current_version, in order.
    ///
    /// Note: migrations are run directly on the connection (not inside an
    /// explicit transaction) because SQLite DDL auto-commits and some
    /// initialisation scripts set `PRAGMA journal_mode = WAL`, which is
    /// illegal inside a transaction.  Partial failures are recoverable by
    /// re-running the runner — the `user_version` pragma is only bumped
    /// after a migration succeeds.
    pub fn run(
        &self,
        conn: &mut rusqlite::Connection,
        current_version: u32,
    ) -> anyhow::Result<u32> {
        let mut applied = current_version;
        for m in self
            .migrations
            .iter()
            .filter(|m| m.version > current_version)
        {
            conn.execute_batch(m.sql)
                .map_err(|e| anyhow::anyhow!("migration {} ({}) failed: {e}", m.version, m.name))?;
            conn.pragma_update(None, "user_version", m.version)?;
            applied = m.version;
        }
        Ok(applied)
    }
}

impl Default for MigrationRunner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runner_applies_only_pending_migrations() {
        let runner = MigrationRunner::new();
        let mut conn = rusqlite::Connection::open_in_memory().unwrap();
        let current: u32 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(current, 0);
        let new_version = runner.run(&mut conn, current).unwrap();
        assert_eq!(new_version, runner.target_version());
        assert!(runner.target_version() > 0);
    }

    #[test]
    fn runner_is_idempotent_on_already_migrated_db() {
        let runner = MigrationRunner::new();
        let mut conn = rusqlite::Connection::open_in_memory().unwrap();
        let current: u32 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        let new_version = runner.run(&mut conn, current).unwrap();
        assert_eq!(new_version, runner.target_version());

        // Second run should be a no-op.
        let current2: u32 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(current2, runner.target_version());
        let new_version2 = runner.run(&mut conn, current2).unwrap();
        assert_eq!(new_version2, runner.target_version());
    }

    #[test]
    fn runner_rollback_on_failing_migration() {
        let mut conn = rusqlite::Connection::open_in_memory().unwrap();

        // Apply version 1 only, simulating a pre-v2 database.
        let initial_runner = MigrationRunner {
            migrations: vec![Migration {
                version: 1,
                name: "initial",
                sql: include_str!("001_initial.sql"),
            }],
        };
        let v1 = initial_runner.run(&mut conn, 0).unwrap();
        assert_eq!(v1, 1);

        // Now try to migrate from 1 to 2 using a runner with a broken migration.
        let broken_runner = MigrationRunner {
            migrations: vec![
                Migration {
                    version: 1,
                    name: "initial",
                    sql: include_str!("001_initial.sql"),
                },
                Migration {
                    version: 2,
                    name: "broken",
                    sql: "THIS IS NOT VALID SQL;",
                },
            ],
        };
        let result = broken_runner.run(&mut conn, 1);
        assert!(result.is_err());

        // user_version should still be 1.
        let current: u32 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(current, 1);
    }
}
