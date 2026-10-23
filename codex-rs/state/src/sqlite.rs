//! Shared SQLite connection configuration.
//! New databases use DELETE on NFS and WAL elsewhere.
//! Writable pools preserve the journal and vacuum modes of initialized databases.
//! Startup retries SQLITE_BUSY and reports other errors to callers.
//! The configured logs directory is created and locked only when opening its database.

#![expect(
    clippy::disallowed_methods,
    reason = "this is the centralized SQLite connection shim"
)]

use crate::DbTelemetry;
use crate::migrations::repair_legacy_recency_migration_version;
use crate::runtime::recovery::RuntimeDbInitError;
use crate::telemetry;
use crate::telemetry::DbKind;
use codex_file_system::LocalLogsDir;
use codex_utils_absolute_path::AbsolutePathBuf;
use log::LevelFilter;
use sqlx::ConnectOptions;
use sqlx::Error;
use sqlx::SqlitePool;
use sqlx::migrate::Migrator;
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::sqlite::SqliteSynchronous;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;
use tokio::sync::OnceCell;

mod busy_retry;
mod validation;

pub use busy_retry::SqlitePoolWriteExt;
pub(crate) use busy_retry::SqliteWriteQueryExt;
pub(crate) use busy_retry::retry_on_busy;
use validation::SqliteQuickCheckManager;

const DEFAULT_BUSY_TIMEOUT: Duration = Duration::from_secs(5);

const LOGS_DB_FILENAME: &str = "logs_2.sqlite";
const GOALS_DB_FILENAME: &str = "goals_1.sqlite";
const MEMORIES_DB_FILENAME: &str = "memories_1.sqlite";
const QUEUE_DB_FILENAME: &str = "queue_1.sqlite";
const STATE_DB_FILENAME: &str = "state_5.sqlite";
const THREAD_HISTORY_DB_FILENAME: &str = "thread_history_1.sqlite";

#[derive(Clone, Copy, Eq, PartialEq)]
enum RecoveryMode {
    BackupAndRebuild,
    Unavailable,
}

#[derive(Clone, Copy)]
struct RuntimeDbSpec {
    label: &'static str,
    filename: &'static str,
    kind: DbKind,
    open_phase: &'static str,
    migrate_phase: &'static str,
    /// Opt in only after auditing writers for deferred read-to-write upgrades:
    /// an intervening reclamation commit can make those fail with SQLITE_BUSY_SNAPSHOT.
    background_reclamation: bool,
    recovery: RecoveryMode,
}

impl RuntimeDbSpec {
    fn path(self, sqlite: &SqliteConfig) -> PathBuf {
        let home = if matches!(self.kind, DbKind::Logs) {
            sqlite.logs_home()
        } else {
            sqlite.home()
        };
        home.join(self.filename)
    }
}

const STATE_DB: RuntimeDbSpec = RuntimeDbSpec {
    label: "state DB",
    filename: STATE_DB_FILENAME,
    kind: DbKind::State,
    open_phase: "open_state",
    migrate_phase: "migrate_state",
    background_reclamation: false,
    recovery: RecoveryMode::BackupAndRebuild,
};

const LOGS_DB: RuntimeDbSpec = RuntimeDbSpec {
    label: "log DB",
    filename: LOGS_DB_FILENAME,
    kind: DbKind::Logs,
    open_phase: "open_logs",
    migrate_phase: "migrate_logs",
    // Log transactions write before reading, so they already hold the writer lock.
    background_reclamation: true,
    recovery: RecoveryMode::BackupAndRebuild,
};

const GOALS_DB: RuntimeDbSpec = RuntimeDbSpec {
    label: "goals DB",
    filename: GOALS_DB_FILENAME,
    kind: DbKind::Goals,
    open_phase: "open_goals",
    migrate_phase: "migrate_goals",
    background_reclamation: false,
    recovery: RecoveryMode::BackupAndRebuild,
};

const MEMORIES_DB: RuntimeDbSpec = RuntimeDbSpec {
    label: "memories DB",
    filename: MEMORIES_DB_FILENAME,
    kind: DbKind::Memories,
    open_phase: "open_memories",
    migrate_phase: "migrate_memories",
    background_reclamation: false,
    recovery: RecoveryMode::BackupAndRebuild,
};

const MEMORIES_V2_DB: RuntimeDbSpec = RuntimeDbSpec {
    label: "memories v2 DB",
    filename: "memories_v2_1.sqlite",
    background_reclamation: false,
    ..MEMORIES_DB
};

const QUEUE_DB: RuntimeDbSpec = RuntimeDbSpec {
    label: "queue DB",
    filename: QUEUE_DB_FILENAME,
    kind: DbKind::Queue,
    open_phase: "open_queue",
    migrate_phase: "migrate_queue",
    background_reclamation: false,
    recovery: RecoveryMode::BackupAndRebuild,
};

const THREAD_HISTORY_DB: RuntimeDbSpec = RuntimeDbSpec {
    label: "thread history DB",
    filename: THREAD_HISTORY_DB_FILENAME,
    kind: DbKind::ThreadHistory,
    open_phase: "open_thread_history",
    migrate_phase: "migrate_thread_history",
    background_reclamation: false,
    recovery: RecoveryMode::Unavailable,
};

const RUNTIME_DBS: [RuntimeDbSpec; 7] = [
    STATE_DB,
    LOGS_DB,
    GOALS_DB,
    MEMORIES_DB,
    MEMORIES_V2_DB,
    QUEUE_DB,
    THREAD_HISTORY_DB,
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeDbPath {
    pub label: &'static str,
    pub path: PathBuf,
    pub(crate) background_reclamation: bool,
}

/// Resolved configuration shared by all Codex SQLite connections.
/// Clones share directory locks, filesystem probes, and quick-check attempts.
#[derive(Clone, Debug)]
pub struct SqliteConfig {
    sqlite_home: AbsolutePathBuf,
    local_logs_dir: Option<LocalLogsDir>,
    is_nfs: Arc<OnceCell<bool>>,
    logs_is_nfs: Arc<OnceCell<bool>>,
    quick_check_manager: SqliteQuickCheckManager,
}

// Directory locks, cached filesystem information, and validation history are operational state,
// not part of configuration equality.
impl PartialEq for SqliteConfig {
    fn eq(&self, other: &Self) -> bool {
        self.sqlite_home == other.sqlite_home
            && self.local_logs_dir.as_ref().map(LocalLogsDir::path)
                == other.local_logs_dir.as_ref().map(LocalLogsDir::path)
    }
}

impl Eq for SqliteConfig {}

impl SqliteConfig {
    pub fn from_sqlite_home(sqlite_home: AbsolutePathBuf) -> Self {
        Self {
            sqlite_home,
            local_logs_dir: None,
            is_nfs: Arc::new(OnceCell::new()),
            logs_is_nfs: Arc::new(OnceCell::new()),
            quick_check_manager: SqliteQuickCheckManager::new(),
        }
    }

    pub fn new_for_testing(sqlite_home: AbsolutePathBuf) -> Self {
        Self::from_sqlite_home(sqlite_home)
    }

    pub fn home(&self) -> &Path {
        self.sqlite_home.as_path()
    }

    /// Select the logs directory without accessing it until its database is opened.
    pub fn with_local_logs_home(mut self, home: PathBuf) -> Self {
        self.local_logs_dir = Some(LocalLogsDir::new(home));
        self.logs_is_nfs = Arc::new(OnceCell::new());
        self
    }

    pub(crate) fn logs_home(&self) -> &Path {
        self.local_logs_dir
            .as_ref()
            .map_or(self.home(), LocalLogsDir::path)
    }

    /// Detects if the sqlite_home directory is under the NFS mount (once per the lifetime)
    pub async fn is_nfs(&self) -> anyhow::Result<bool> {
        probe_nfs(self.home(), &self.is_nfs).await
    }

    pub(crate) async fn logs_are_on_nfs(&self) -> anyhow::Result<bool> {
        probe_nfs(self.logs_home(), &self.logs_is_nfs).await
    }

    /// Return the path to the primary state database.
    pub fn state_db_path(&self) -> PathBuf {
        STATE_DB.path(self)
    }

    /// Return the path to the logs database.
    pub fn logs_db_path(&self) -> PathBuf {
        LOGS_DB.path(self)
    }

    pub(crate) fn legacy_logs_db_path(&self) -> Option<PathBuf> {
        let path = self.home().join(LOGS_DB_FILENAME);
        (path != self.logs_db_path()).then_some(path)
    }

    /// Return the path to the goals database.
    pub fn goals_db_path(&self) -> PathBuf {
        GOALS_DB.path(self)
    }

    /// Return the path to the memories database.
    pub fn memories_db_path(&self) -> PathBuf {
        MEMORIES_DB.path(self)
    }

    pub(crate) fn memories_v2_db_path(&self) -> PathBuf {
        MEMORIES_V2_DB.path(self)
    }

    pub(crate) async fn open_memories_v2_db(&self) -> anyhow::Result<SqlitePool> {
        self.open_runtime_db(
            MEMORIES_V2_DB,
            &crate::migrations::runtime_memories_migrator(),
            /*telemetry_override*/ None,
        )
        .await
    }

    /// Return the path to the durable user-message queue database.
    pub fn queue_db_path(&self) -> PathBuf {
        QUEUE_DB.path(self)
    }

    /// Return the path to the paginated thread-history database.
    pub fn thread_history_db_path(&self) -> PathBuf {
        THREAD_HISTORY_DB.path(self)
    }

    /// Return the paths to every database managed by the state runtime.
    pub fn runtime_db_paths(&self) -> Vec<RuntimeDbPath> {
        RUNTIME_DBS
            .iter()
            .map(|spec| RuntimeDbPath {
                label: spec.label,
                path: spec.path(self),
                background_reclamation: spec.background_reclamation,
            })
            .collect()
    }

    pub(crate) fn reclamation_db_paths(&self) -> Vec<RuntimeDbPath> {
        self.runtime_db_paths()
            .into_iter()
            .filter(|db| db.background_reclamation)
            .chain(self.legacy_logs_db_path().map(|path| RuntimeDbPath {
                label: "legacy log DB",
                path,
                background_reclamation: true,
            }))
            .collect()
    }

    pub(super) async fn open_state_db(
        &self,
        migrator: &Migrator,
        telemetry_override: Option<&dyn DbTelemetry>,
    ) -> anyhow::Result<SqlitePool> {
        // New state DBs should use incremental auto-vacuum, but retrofitting an
        // existing DB requires a full VACUUM. Do not attempt that during process
        // startup: it is maintenance work that can contend with foreground writers.
        self.open_runtime_db(STATE_DB, migrator, telemetry_override)
            .await
    }

    pub(super) async fn open_logs_db(
        &self,
        migrator: &Migrator,
        telemetry_override: Option<&dyn DbTelemetry>,
    ) -> anyhow::Result<SqlitePool> {
        self.open_runtime_db(LOGS_DB, migrator, telemetry_override)
            .await
    }

    pub(super) async fn open_goals_db(
        &self,
        migrator: &Migrator,
        telemetry_override: Option<&dyn DbTelemetry>,
    ) -> anyhow::Result<SqlitePool> {
        self.open_runtime_db(GOALS_DB, migrator, telemetry_override)
            .await
    }

    pub(super) async fn open_memories_db(
        &self,
        migrator: &Migrator,
        telemetry_override: Option<&dyn DbTelemetry>,
    ) -> anyhow::Result<SqlitePool> {
        self.open_runtime_db(MEMORIES_DB, migrator, telemetry_override)
            .await
    }

    pub(super) async fn open_queue_db(
        &self,
        migrator: &Migrator,
        telemetry_override: Option<&dyn DbTelemetry>,
    ) -> anyhow::Result<SqlitePool> {
        self.open_runtime_db(QUEUE_DB, migrator, telemetry_override)
            .await
    }

    pub(super) async fn open_thread_history_db(
        &self,
        migrator: &Migrator,
        telemetry_override: Option<&dyn DbTelemetry>,
    ) -> anyhow::Result<SqlitePool> {
        self.open_runtime_db(THREAD_HISTORY_DB, migrator, telemetry_override)
            .await
    }

    async fn open_runtime_db(
        &self,
        spec: RuntimeDbSpec,
        migrator: &Migrator,
        telemetry_override: Option<&dyn DbTelemetry>,
    ) -> anyhow::Result<SqlitePool> {
        let path = spec.path(self);
        let started = Instant::now();
        let pool_result = self
            .open_read_write_pool_with_spec(path.as_path(), Some(spec), telemetry_override)
            .await;
        telemetry::record_init_result(
            telemetry_override,
            spec.kind,
            spec.open_phase,
            started.elapsed(),
            &pool_result,
        );
        let pool = pool_result.map_err(|source| {
            RuntimeDbInitError::new(spec.label, "open", path.as_path(), source)
        })?;
        let started = Instant::now();
        let migrate_result = async {
            if matches!(spec.kind, DbKind::State) {
                repair_legacy_recency_migration_version(&pool, migrator).await?;
            }
            migrator.run(&pool).await.map_err(anyhow::Error::from)
        }
        .await;
        telemetry::record_init_result(
            telemetry_override,
            spec.kind,
            spec.migrate_phase,
            started.elapsed(),
            &migrate_result,
        );
        if let Err(source) = migrate_result {
            pool.close().await;
            return Err(
                RuntimeDbInitError::new(spec.label, "migrate", path.as_path(), source).into(),
            );
        }
        Ok(pool)
    }

    /// Open a writable Codex SQLite database, creating it if necessary.
    /// Keep this configuration alive to retain the cleanup lock for local logs.
    pub async fn open_read_write_pool(&self, path: &Path) -> anyhow::Result<SqlitePool> {
        self.open_read_write_pool_with_spec(
            path, /*recover_spec*/ None, /*telemetry_override*/ None,
        )
        .await
    }

    async fn open_read_write_pool_with_spec(
        &self,
        path: &Path,
        recover_spec: Option<RuntimeDbSpec>,
        telemetry_override: Option<&dyn DbTelemetry>,
    ) -> anyhow::Result<SqlitePool> {
        self.prepare_logs_directory(path).await?;
        let directory = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let is_nfs = if self.local_logs_dir.is_some() && directory == self.logs_home() {
            self.logs_are_on_nfs().await?
        } else if directory == self.home() {
            self.is_nfs().await?
        } else {
            probe_nfs(directory, &OnceCell::new()).await?
        };
        let journal_mode_policy = if is_nfs {
            JournalModePolicy::InitializeDelete
        } else {
            JournalModePolicy::InitializeWal
        };
        let recovery = recover_spec.map_or(RecoveryMode::Unavailable, |spec| spec.recovery);
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .synchronous(SqliteSynchronous::Normal)
            .busy_timeout(DEFAULT_BUSY_TIMEOUT)
            .log_statements(LevelFilter::Off);
        // Each failed attempt closes its pool before initialization is retried.
        let connect = || async {
            connect_once(&options, journal_mode_policy)
                .await
                .map_err(anyhow::Error::from)
        };
        let pool = retry_on_busy(connect).await?;
        let validation_result = self
            .quick_check_manager
            .quick_check_once(
                &pool,
                path,
                // Limit startup validation to 100 ms.
                Duration::from_millis(/*millis*/ 100),
            )
            .await;

        let Ok(result) = validation_result else {
            pool.close().await;
            return validation_result.map(|_| pool);
        };

        if result == validation::QuickCheckOutcome::CorruptedNeedsFixed {
            tracing::error!(database = %path.display(), "SQLite quick check detected corruption");
            telemetry::record_corruption(telemetry_override, recover_spec.map(|spec| spec.kind));
            if recovery == RecoveryMode::BackupAndRebuild {
                pool.close().await;
                let backups = crate::backup_runtime_db_for_fresh_start(path).await?;
                for backup in backups {
                    tracing::warn!(
                        database = %backup.original_path.display(),
                        backup = %backup.backup_path.display(),
                        "Preserved corrupt SQLite database before rebuilding"
                    );
                }

                return retry_on_busy(connect).await;
            }
        }

        Ok(pool)
    }

    /// Open an existing Codex SQLite database without creating or modifying it.
    /// Keep this configuration alive to retain the cleanup lock for local logs.
    pub async fn open_read_only_pool(
        &self,
        path: &Path,
        busy_timeout: Option<Duration>,
    ) -> Result<SqlitePool, Error> {
        self.prepare_logs_directory(path).await.map_err(Error::Io)?;
        let mut options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(false)
            .read_only(true)
            .log_statements(LevelFilter::Off);
        if let Some(busy_timeout) = busy_timeout {
            options = options.busy_timeout(busy_timeout);
        }
        SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
    }

    /// Open old logs for cleanup without creating, migrating, or rebuilding the database.
    pub(crate) async fn open_legacy_logs_for_cleanup(&self) -> anyhow::Result<Option<SqlitePool>> {
        let Some(path) = self.legacy_logs_db_path() else {
            return Ok(None);
        };
        if !tokio::fs::try_exists(&path).await? {
            return Ok(None);
        }
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(false)
            .busy_timeout(Duration::ZERO)
            .log_statements(LevelFilter::Off);
        SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .map(Some)
            .map_err(anyhow::Error::from)
    }

    async fn prepare_logs_directory(&self, path: &Path) -> std::io::Result<()> {
        // Login, login status, and logout load configuration without opening SQLite.
        // Create and lock the logs directory only when opening its database.
        // An unavailable log_dir must not prevent those commands from loading configuration.
        if let Some(directory) = &self.local_logs_dir
            && path == self.logs_db_path()
        {
            directory.initialize().await?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum JournalModePolicy {
    InitializeWal,
    InitializeDelete,
}

async fn connect_once(
    options: &SqliteConnectOptions,
    journal_mode_policy: JournalModePolicy,
) -> Result<SqlitePool, Error> {
    // SQLx retries after_connect errors, eventually replacing them with PoolTimedOut.
    // Return the first initialization error directly while opening this pool.
    let (init_error_tx, mut init_error_rx) = tokio::sync::mpsc::channel(/*buffer*/ 1);
    let pool_options =
        SqlitePoolOptions::new()
            .max_connections(5)
            .after_connect(move |connection, _metadata| {
                let init_error_tx = init_error_tx.clone();
                Box::pin(async move {
                    let result = async {
                        let journal: String = sqlx::query_scalar("PRAGMA journal_mode")
                            .fetch_one(&mut *connection)
                            .await?;
                        if journal == "delete" {
                            // If the filesystem honors sync requests, EXTRA syncs the directory
                            // after SQLite deletes a rollback journal.
                            sqlx::query("PRAGMA synchronous = EXTRA")
                                .execute(connection)
                                .await?;
                        }
                        Ok(())
                    }
                    .await;
                    result.map_err(|error| match init_error_tx.try_send(error) {
                        // The opener owns the original error and cancels connection retries.
                        Ok(()) => Error::PoolClosed,
                        // After opening, lazy connections retain SQLx's normal error handling.
                        Err(error) => error.into_inner(),
                    })
                })
            });
    let pool = tokio::select! {
        biased;
        Some(error) = init_error_rx.recv() => Err(error),
        result = pool_options.connect_with(options.clone()) => result,
    }?;
    // Database settings persist on disk.
    // Initialize these settings once before callers can use the pool.
    // Connections that open later need only settings that apply to each connection.
    let initialized: Result<(), Error> = async {
        let mut connection = pool.acquire().await?;
        // Auto-vacuum alone can write a header to disk.
        // If creation stops before the schema exists, startup must initialize the file.
        let empty =
            sqlx::query_scalar::<_, bool>("SELECT NOT EXISTS (SELECT 1 FROM sqlite_schema)")
                .fetch_one(&mut *connection)
                .await?;
        if empty {
            let mode: i64 = sqlx::query_scalar("PRAGMA auto_vacuum")
                .fetch_one(&mut *connection)
                .await?;
            if mode == 0 {
                sqlx::query("PRAGMA auto_vacuum = INCREMENTAL")
                    .execute(&mut *connection)
                    .await?;
            }
            let pragma = match journal_mode_policy {
                JournalModePolicy::InitializeWal => "PRAGMA journal_mode = WAL",
                JournalModePolicy::InitializeDelete => "PRAGMA journal_mode = DELETE",
            };
            sqlx::query(pragma).execute(&mut *connection).await?;
        }
        // Initialization or another process can change the journal mode after after_connect.
        // Match this connection's synchronization setting to the resulting journal mode.
        let journal: String = sqlx::query_scalar("PRAGMA journal_mode")
            .fetch_one(&mut *connection)
            .await?;
        if journal == "wal" {
            sqlx::query("PRAGMA synchronous = NORMAL")
                .execute(&mut *connection)
                .await?;
        } else if journal == "delete" {
            sqlx::query("PRAGMA synchronous = EXTRA")
                .execute(&mut *connection)
                .await?;
        }
        // Wait for the connection to return to the pool before startup requests it again.
        // An asynchronous drop can otherwise cause the pool to open a second connection.
        connection.return_to_pool().await;
        Ok(())
    }
    .await;
    if let Err(error) = initialized {
        pool.close().await;
        return Err(error);
    }
    Ok(pool)
}

async fn probe_nfs(home: &Path, probe: &OnceCell<bool>) -> anyhow::Result<bool> {
    probe
        .get_or_try_init(|| async {
            let home = home.to_path_buf();
            Ok(tokio::task::spawn_blocking(move || codex_file_system::is_nfs(&home)).await??)
        })
        .await
        .copied()
}

#[cfg(test)]
#[path = "sqlite_tests.rs"]
mod tests;
