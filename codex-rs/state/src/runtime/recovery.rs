//! Backup-and-rebuild support for Codex runtime SQLite databases.
//!
//! Codex keeps several independent runtime SQLite databases under one SQLite
//! home. When SQLite reports that one of them is corrupt, automatic recovery
//! moves only that database file and its sidecars into a backup folder so the
//! other databases keep their data.

use super::RUNTIME_DBS;
use std::path::Path;
use std::path::PathBuf;

const BACKUP_DIR_NAME: &str = "db-backups";

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
    match tokio::fs::metadata(sqlite_home).await {
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
    }
}

/// Move every Codex runtime SQLite file out of the way.
///
/// This is intended for explicit full-reset flows. Automatic corruption
/// recovery should use [`backup_runtime_db_for_fresh_start`] so unrelated
/// runtime databases are preserved.
pub async fn backup_runtime_dbs_for_fresh_start(
    sqlite_home: &Path,
) -> std::io::Result<Vec<RuntimeDbBackup>> {
    match tokio::fs::metadata(sqlite_home).await {
        Ok(metadata) if metadata.is_dir() => backup_all_runtime_db_files(sqlite_home).await,
        Ok(_) => backup_blocking_sqlite_home(sqlite_home).await,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            tokio::fs::create_dir_all(sqlite_home).await?;
            Err(std::io::Error::other(
                "no Codex runtime database files were found to back up",
            ))
        }
        Err(err) => Err(err),
    }
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
    err.chain().any(|source| {
        let detail = source.to_string();
        sqlite_error_detail_is_corruption(&detail)
    })
}

pub fn sqlite_error_detail_is_corruption(detail: &str) -> bool {
    let detail = detail.to_ascii_lowercase();
    detail.contains("database disk image is malformed")
        || detail.contains("database schema is malformed")
        || detail.contains("database is corrupt")
        || detail.contains("file is not a database")
        || detail.contains("sqlite_corrupt")
        || detail.contains("sqlite_notadb")
        || detail.contains("(code: 11)")
        || detail.contains("(code: 26)")
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

async fn backup_all_runtime_db_files(sqlite_home: &Path) -> std::io::Result<Vec<RuntimeDbBackup>> {
    let paths = RUNTIME_DBS
        .iter()
        .map(|spec| spec.path(sqlite_home))
        .flat_map(|path| sqlite_paths(path.as_path()));
    backup_sqlite_paths(sqlite_home, paths).await
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
mod tests {
    use super::*;
    use crate::runtime::test_support::unique_temp_dir;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn backup_moves_only_requested_runtime_db_files_to_backup_folder() -> std::io::Result<()>
    {
        let sqlite_home = unique_temp_dir();
        tokio::fs::create_dir_all(sqlite_home.as_path()).await?;
        let runtime_paths = super::super::runtime_db_paths(sqlite_home.as_path());
        let mut expected_paths = Vec::new();
        for db_path in runtime_paths.iter().map(|db| db.path.as_path()) {
            for path in sqlite_paths(db_path) {
                tokio::fs::write(path.as_path(), path.display().to_string()).await?;
                expected_paths.push(path);
            }
        }
        let failed_db_path = super::super::logs_db_path(sqlite_home.as_path());
        let failed_paths = sqlite_paths(failed_db_path.as_path());

        let backups = backup_runtime_db_for_fresh_start(failed_db_path.as_path()).await?;

        assert_eq!(backups.len(), failed_paths.len());
        for path in &failed_paths {
            assert!(!tokio::fs::try_exists(path.as_path()).await?);
        }
        for path in expected_paths
            .iter()
            .filter(|path| !failed_paths.contains(path))
        {
            assert!(tokio::fs::try_exists(path.as_path()).await?);
        }
        for backup in backups {
            assert!(
                backup
                    .backup_path
                    .starts_with(sqlite_home.join(BACKUP_DIR_NAME))
            );
            assert!(tokio::fs::try_exists(backup.backup_path.as_path()).await?);
        }
        let _ = tokio::fs::remove_dir_all(sqlite_home).await;
        Ok(())
    }

    #[tokio::test]
    async fn backup_all_runtime_db_files_to_backup_folder() -> std::io::Result<()> {
        let sqlite_home = unique_temp_dir();
        tokio::fs::create_dir_all(sqlite_home.as_path()).await?;
        let runtime_paths = super::super::runtime_db_paths(sqlite_home.as_path());
        let mut expected_paths = Vec::new();
        for db_path in runtime_paths.iter().map(|db| db.path.as_path()) {
            for path in sqlite_paths(db_path) {
                tokio::fs::write(path.as_path(), path.display().to_string()).await?;
                expected_paths.push(path);
            }
        }

        let backups = backup_runtime_dbs_for_fresh_start(sqlite_home.as_path()).await?;

        assert_eq!(backups.len(), expected_paths.len());
        for path in &expected_paths {
            assert!(!tokio::fs::try_exists(path.as_path()).await?);
        }
        for backup in backups {
            assert!(
                backup
                    .backup_path
                    .starts_with(sqlite_home.join(BACKUP_DIR_NAME))
            );
            assert!(tokio::fs::try_exists(backup.backup_path.as_path()).await?);
        }
        let _ = tokio::fs::remove_dir_all(sqlite_home).await;
        Ok(())
    }

    #[tokio::test]
    async fn backup_replaces_blocking_sqlite_home_file() -> std::io::Result<()> {
        let temp_dir = unique_temp_dir();
        tokio::fs::create_dir_all(temp_dir.as_path()).await?;
        let sqlite_home = temp_dir.join("sqlite-home");
        tokio::fs::write(sqlite_home.as_path(), b"not-a-directory").await?;

        let backups = backup_runtime_dbs_for_fresh_start(sqlite_home.as_path()).await?;

        assert_eq!(backups.len(), 1);
        assert!(tokio::fs::metadata(sqlite_home.as_path()).await?.is_dir());
        assert!(
            backups[0]
                .backup_path
                .starts_with(temp_dir.join(format!("sqlite-home.{BACKUP_DIR_NAME}")))
        );
        assert!(tokio::fs::try_exists(backups[0].backup_path.as_path()).await?);
        let _ = tokio::fs::remove_dir_all(temp_dir).await;
        Ok(())
    }

    #[test]
    fn sqlite_error_detail_classifies_corruption_and_lock_errors() {
        assert!(sqlite_error_detail_is_corruption(
            "database disk image is malformed"
        ));
        assert!(sqlite_error_detail_is_corruption("file is not a database"));
        assert!(sqlite_error_detail_is_corruption(
            "error returned from database: (code: 11) database disk image is malformed"
        ));
        assert!(!sqlite_error_detail_is_corruption("database is locked"));
        assert!(sqlite_error_detail_is_lock("database is locked"));
        assert!(sqlite_error_detail_is_lock("database is busy"));
        assert!(!sqlite_error_detail_is_lock(
            "database disk image is malformed"
        ));
    }

    #[test]
    fn runtime_db_path_for_corruption_error_returns_failed_database_path() {
        let path = PathBuf::from("/tmp/logs_2.sqlite");
        let err = anyhow::Error::new(RuntimeDbInitError::new(
            "log DB",
            "open",
            path.as_path(),
            anyhow::anyhow!("database disk image is malformed"),
        ));

        assert_eq!(runtime_db_path_for_corruption_error(&err), Some(path));
    }
}
