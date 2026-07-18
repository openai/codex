//! Shared SQLite connection configuration.

use codex_utils_absolute_path::AbsolutePathBuf;
use log::LevelFilter;
use sqlx::ConnectOptions;
use sqlx::Error;
use sqlx::SqlitePool;
use sqlx::sqlite::SqliteAutoVacuum;
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::sqlite::SqliteJournalMode;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::sqlite::SqliteSynchronous;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

/// Resolved configuration shared by all Codex SQLite connections.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SqliteConfig {
    sqlite_home: AbsolutePathBuf,
}

impl SqliteConfig {
    pub fn from_sqlite_home(sqlite_home: AbsolutePathBuf) -> Self {
        Self { sqlite_home }
    }

    #[expect(
        clippy::expect_used,
        reason = "test sqlite homes must already be absolute"
    )]
    pub fn new_for_testing(sqlite_home: PathBuf) -> Self {
        Self::from_sqlite_home(
            AbsolutePathBuf::try_from(sqlite_home).expect("sqlite home should be absolute"),
        )
    }

    pub fn home(&self) -> &Path {
        self.sqlite_home.as_path()
    }

    /// Open a writable Codex SQLite database, creating it if necessary.
    pub async fn open_read_write_pool(&self, path: &Path) -> Result<SqlitePool, Error> {
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal)
            .auto_vacuum(SqliteAutoVacuum::Incremental)
            .busy_timeout(Duration::from_secs(5))
            .log_statements(LevelFilter::Off);
        SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await
    }

    /// Open an existing Codex SQLite database without creating or modifying it.
    pub async fn open_read_only_pool(&self, path: &Path) -> Result<SqlitePool, Error> {
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(false)
            .read_only(true)
            .log_statements(LevelFilter::Off);
        SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
    }
}
