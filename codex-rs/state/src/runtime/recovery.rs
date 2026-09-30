//! Backup-and-rebuild support for Codex runtime SQLite databases.
//!
//! Codex keeps several independent runtime SQLite databases under one SQLite
//! home. When SQLite reports that one of them is corrupt, automatic recovery
//! moves only that database file and its sidecars into a backup folder so the
//! other databases keep their data. Startup callers may collect backups in a
//! task-local scope to notify users after recovery; backup behavior is unchanged
//! outside that scope.

use sqlx::error::DatabaseError;
use std::cell::RefCell;
use std::future::Future;
use std::path::Path;
use std::path::PathBuf;

const BACKUP_DIR_NAME: &str = "db-backups";

tokio::task_local! {
    static RECOVERY_BACKUPS: RefCell<Vec<RuntimeDbBackup>>;
}

/// Collect backups made while polling startup, including recovery across retries.
///
/// The report is scoped to this task: unrelated processes and tasks are excluded.
/// Recovery in spawned tasks is not collected. Callers should report successful
/// recovery only after startup has completed successfully.
pub async fn collect_runtime_db_backups<T>(
    startup: impl Future<Output = T>,
) -> (T, Vec<RuntimeDbBackup>) {
    RECOVERY_BACKUPS
        .scope(RefCell::new(Vec::new()), async {
            let result = startup.await;
            let backups = RECOVERY_BACKUPS.with(std::cell::RefCell::take);
            (result, backups)
        })
        .await
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeDbBackup {
    /// Path where the runtime database or sidecar lived before it was moved.
    pub original_path: PathBuf,
    /// Path where the runtime database or sidecar was backed up.
    pub backup_path: PathBuf,
}

#[derive(Debug)]
pub(crate) struct RuntimeDbInitError {
    label: &'static str,
    operation: &'static str,
    path: PathBuf,
    source: anyhow::Error,
}

impl RuntimeDbInitError {
    pub(crate) fn new(
        label: &'static str,
        operation: &'static str,
        path: &Path,
        source: anyhow::Error,
    ) -> Self {
        Self {
            label,
            operation,
            path: path.to_path_buf(),
            source,
        }
    }

    fn path(&self) -> &Path {
        self.path.as_path()
    }
}

impl std::fmt::Display for RuntimeDbInitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "failed to {} {} at {}: {}",
            self.operation,
            self.label,
            self.path.display(),
            self.source
        )
    }
}

impl std::error::Error for RuntimeDbInitError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.source.as_ref())
    }
}

/// Move one Codex runtime SQLite database out of the way so that database can
/// be recreated without discarding unrelated runtime databases.
pub async fn backup_runtime_db_for_fresh_start(
    db_path: &Path,
) -> std::io::Result<Vec<RuntimeDbBackup>> {
    let sqlite_home = db_path.parent().ok_or_else(|| {
        std::io::Error::other(format!(
            "database path does not have a parent directory: {}",
            db_path.display()
        ))
    })?;
    let backups = match tokio::fs::metadata(sqlite_home).await {
        Ok(metadata) if metadata.is_dir() => backup_runtime_db_files(db_path).await,
        Ok(_) => backup_blocking_sqlite_home(sqlite_home).await,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            tokio::fs::create_dir_all(sqlite_home).await?;
            Err(std::io::Error::other(format!(
                "no Codex runtime database files were found to back up for {}",
                db_path.display()
            )))
        }
        Err(err) => Err(err),
    }?;
    let _ = RECOVERY_BACKUPS.try_with(|report| report.borrow_mut().extend(backups.iter().cloned()));
    Ok(backups)
}

pub fn runtime_db_path_for_corruption_error(err: &anyhow::Error) -> Option<PathBuf> {
    if !is_sqlite_corruption_error(err) {
        return None;
    }
    err.chain()
        .find_map(|source| source.downcast_ref::<RuntimeDbInitError>())
        .map(|err| err.path().to_path_buf())
}

pub fn is_sqlite_corruption_error(err: &anyhow::Error) -> bool {
    err.chain().any(sqlite_error_source_is_corruption)
}

fn sqlite_error_source_is_corruption(source: &(dyn std::error::Error + 'static)) -> bool {
    if let Some(err) = source.downcast_ref::<libsqlite3_sys::Error>() {
        return matches!(
            err.code,
            libsqlite3_sys::ErrorCode::DatabaseCorrupt | libsqlite3_sys::ErrorCode::NotADatabase
        );
    }

    let sqlite_error = source
        .downcast_ref::<sqlx::sqlite::SqliteError>()
        .or_else(|| {
            source
                .downcast_ref::<sqlx::Error>()?
                .as_database_error()?
                .try_downcast_ref::<sqlx::sqlite::SqliteError>()
        });
    let Some(err) = sqlite_error else {
        return false;
    };

    // SQLx exposes SQLite's extended result code as a decimal string.
    err.code()
        .and_then(|code| code.parse::<i32>().ok())
        .is_some_and(|code| {
            matches!(
                libsqlite3_sys::Error::new(code).code,
                libsqlite3_sys::ErrorCode::DatabaseCorrupt
                    | libsqlite3_sys::ErrorCode::NotADatabase
            )
        })
}

pub fn sqlite_error_detail_is_lock(detail: &str) -> bool {
    let detail = detail.to_ascii_lowercase();
    detail.contains("database is locked") || detail.contains("database is busy")
}

async fn backup_runtime_db_files(db_path: &Path) -> std::io::Result<Vec<RuntimeDbBackup>> {
    let sqlite_home = db_path.parent().ok_or_else(|| {
        std::io::Error::other(format!(
            "database path does not have a parent directory: {}",
            db_path.display()
        ))
    })?;
    backup_sqlite_paths(sqlite_home, sqlite_paths(db_path)).await
}

async fn backup_sqlite_paths(
    sqlite_home: &Path,
    paths: impl IntoIterator<Item = PathBuf>,
) -> std::io::Result<Vec<RuntimeDbBackup>> {
    let backup_dir = create_unique_backup_dir(sqlite_home.join(BACKUP_DIR_NAME).as_path()).await?;
    let mut backups = Vec::new();

    for path in paths {
        if tokio::fs::try_exists(path.as_path()).await? {
            let backup_path = backup_dir.join(file_name(path.as_path())?);
            tokio::fs::rename(path.as_path(), backup_path.as_path()).await?;
            backups.push(RuntimeDbBackup {
                original_path: path,
                backup_path,
            });
        }
    }

    if backups.is_empty() {
        let _ = tokio::fs::remove_dir(backup_dir).await;
        return Err(std::io::Error::other(
            "no Codex runtime database files were found to back up",
        ));
    }

    Ok(backups)
}

async fn backup_blocking_sqlite_home(sqlite_home: &Path) -> std::io::Result<Vec<RuntimeDbBackup>> {
    let parent = sqlite_home.parent().ok_or_else(|| {
        std::io::Error::other(format!(
            "cannot create a backup folder for {}",
            sqlite_home.display()
        ))
    })?;
    let mut backup_dir_name = file_name(sqlite_home)?.to_os_string();
    backup_dir_name.push(format!(".{BACKUP_DIR_NAME}"));
    let backup_parent = parent.join(backup_dir_name);
    let backup_dir = create_unique_backup_dir(backup_parent.as_path()).await?;
    let backup_path = backup_dir.join(file_name(sqlite_home)?);
    tokio::fs::rename(sqlite_home, backup_path.as_path()).await?;
    tokio::fs::create_dir_all(sqlite_home).await?;
    Ok(vec![RuntimeDbBackup {
        original_path: sqlite_home.to_path_buf(),
        backup_path,
    }])
}

fn sqlite_paths(db_path: &Path) -> Vec<PathBuf> {
    let mut wal_path = db_path.as_os_str().to_os_string();
    wal_path.push("-wal");
    let mut shm_path = db_path.as_os_str().to_os_string();
    shm_path.push("-shm");
    vec![
        db_path.to_path_buf(),
        PathBuf::from(wal_path),
        PathBuf::from(shm_path),
    ]
}

async fn create_unique_backup_dir(backup_parent: &Path) -> std::io::Result<PathBuf> {
    tokio::fs::create_dir_all(backup_parent).await?;
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs());
    let mut sequence = 0_u32;
    loop {
        let backup_dir = backup_parent.join(format!("sqlite-{timestamp}-{sequence}"));
        match tokio::fs::create_dir(backup_dir.as_path()).await {
            Ok(()) => return Ok(backup_dir),
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                sequence += 1;
            }
            Err(err) => return Err(err),
        }
    }
}

fn file_name(path: &Path) -> std::io::Result<&std::ffi::OsStr> {
    path.file_name().ok_or_else(|| {
        std::io::Error::other(format!(
            "cannot create a backup name for {}",
            path.display()
        ))
    })
}

#[cfg(test)]
#[path = "recovery_tests.rs"]
mod tests;
