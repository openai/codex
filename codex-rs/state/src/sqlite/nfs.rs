//! Migrates databases after the caller checks the managed policy and detects NFS.
//! Migration errors must not trigger corruption recovery that would discard a WAL backup.

use super::SqliteConfig;
use crate::NfsDbMigration;
use crate::NfsDbMigrationOutcome;
use anyhow::Context;
use std::fs::TryLockError;
use std::path::Path;
use std::time::Duration;
use tokio::time::Instant;

pub(super) async fn prepare_database(
    sqlite: &SqliteConfig,
    path: &Path,
) -> anyhow::Result<NfsDbMigration> {
    // Replacement syncs the source directory, which must not be an empty relative parent.
    let absolute_path = std::path::absolute(path)?;
    let path = absolute_path.as_path();
    let home = path
        .parent()
        .context("database path has no parent directory")?;
    // Concurrent managed startups can briefly hold this lock even after migration is complete.
    let deadline = Instant::now() + super::DEFAULT_BUSY_TIMEOUT;
    let mut migration = loop {
        match NfsDbMigration::lock(home) {
            Ok(migration) => break migration,
            Err(error)
                if matches!(
                    error.downcast_ref::<TryLockError>(),
                    Some(TryLockError::WouldBlock)
                ) && Instant::now() < deadline =>
            {
                tokio::time::sleep(Duration::from_millis(/*millis*/ 100)).await;
            }
            Err(error) => return Err(error),
        }
    };
    // The primary runtime opens state first. Include lazy databases so startup also
    // resumes interrupted migrations whose source path has not been reinstalled yet.
    // A separate logs directory is checked and locked when its own database opens.
    let paths = if path == sqlite.state_db_path() {
        sqlite
            .runtime_db_paths()
            .into_iter()
            .map(|db| db.path)
            .filter(|path| path.parent() == Some(home))
            .collect()
    } else {
        vec![path.to_path_buf()]
    };
    for path in paths {
        let outcome = migration.migrate(&path, &std::env::temp_dir()).await.map_err(|error| {
            // Do not expose a SQLite corruption error to automatic backup-and-rebuild recovery:
            // an interrupted cutover must be resumed by the migration implementation itself.
            anyhow::anyhow!(
                "required NFS database migration failed for {}: {error:#}. Stop Codex instances on all hosts and rerun codex migrate-nfs-db",
                path.display()
            )
        })?;
        if let NfsDbMigrationOutcome::Migrated { backup_directory } = outcome {
            tracing::info!(database = %path.display(), backup = %backup_directory.display(), "Migrated NFS database to DELETE journaling");
        }
    }
    Ok(migration)
}
