//! Converts WAL databases to DELETE mode while preserving recoverable WAL backups.
//! Callers serialize startup and migration with the directory lock. Other SQLite clients
//! must remain stopped during replacement; NFS must enforce cross-client POSIX locks.

use anyhow::Context;
use anyhow::ensure;
use sqlx::ConnectOptions;
use sqlx::Connection;
use sqlx::SqliteConnection;
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::sqlite::SqliteJournalMode;
use sqlx::sqlite::SqliteLockingMode;
use sqlx::sqlite::SqliteSynchronous;
use std::fs::File;
use std::io::ErrorKind;
use std::io::Read;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

#[cfg(unix)]
mod access;
mod replacement;

use replacement::Recovery;
use replacement::Replacement;
use replacement::regular_file_exists;

/// Holds the directory lock shared by automatic startup and the migration command.
pub struct NfsDbMigration {
    _lock: File,
}

/// Result of checking or converting a database, for caller-owned reporting.
pub enum NfsDbMigrationOutcome {
    Missing,
    AlreadyDelete,
    Migrated { backup_directory: PathBuf },
}

impl NfsDbMigration {
    pub fn lock(sqlite_home: &Path) -> anyhow::Result<Self> {
        ensure!(
            cfg!(unix),
            "NFS database migration requires Unix filesystem locking and directory fsync"
        );
        // Never remove the lock: concurrent openers must lock the same inode.
        let lock = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(sqlite_home.join(".codex-nfs-migration.lock"))?;
        lock.try_lock()
            .context("another database migration is running")?;
        Ok(Self { _lock: lock })
    }

    /// Convert an existing WAL database or resume an interrupted replacement.
    /// All other clients must remain stopped until this returns successfully.
    #[expect(
        clippy::disallowed_methods,
        reason = "Migration needs a dedicated exclusive connection, not a runtime pool"
    )]
    pub async fn migrate(
        &mut self,
        source: &Path,
        staging_dir: &Path,
    ) -> anyhow::Result<NfsDbMigrationOutcome> {
        let replacement = Replacement::new(source)?;
        if let Recovery::Installed = replacement.resume().await? {
            return Ok(NfsDbMigrationOutcome::Migrated {
                backup_directory: replacement.backup_directory,
            });
        }
        let exists = regular_file_exists(source)?;
        for suffix in ["-wal", "-shm", "-journal"] {
            let mut path = source.as_os_str().to_owned();
            path.push(suffix);
            let sidecar_exists = regular_file_exists(Path::new(&path))?;
            ensure!(
                exists || !sidecar_exists,
                "database is missing but a SQLite sidecar exists"
            );
        }
        if !exists {
            return Ok(NfsDbMigrationOutcome::Missing);
        }
        let options = SqliteConnectOptions::new()
            .filename(source)
            // Set this mode before SQLite first accesses the WAL.
            // SQLite then locks the database file without shared memory across NFS client kernels.
            .locking_mode(SqliteLockingMode::Exclusive)
            .synchronous(SqliteSynchronous::Full)
            .busy_timeout(Duration::ZERO)
            .disable_statement_logging();
        let mut connection = SqliteConnection::connect_with(&options).await.context(
            "cannot open source database exclusively; close its SQLite clients on all hosts",
        )?;

        // Closing any descriptor for an inode can release this process's POSIX record locks
        // for that inode.
        // Keep both source files open until SQLite closes its connection, including after an error.
        let mut source_files = Vec::new();
        let result: anyhow::Result<bool> = async {
            {
                let mut handle = connection.lock_handle().await?;
                // SAFETY: SQLx's guard prevents its worker from accessing this open connection.
                // This db_config option accepts an int and an optional output pointer to an int.
                let status = unsafe {
                    libsqlite3_sys::sqlite3_db_config(
                        handle.as_raw_handle().as_ptr(),
                        libsqlite3_sys::SQLITE_DBCONFIG_NO_CKPT_ON_CLOSE,
                        /*onoff*/ 1,
                        std::ptr::null_mut::<i32>(),
                    )
                };
                ensure!(
                    status == libsqlite3_sys::SQLITE_OK,
                    "cannot disable source checkpoint on close: {status}"
                );
            }
            let mode: String = sqlx::query_scalar("PRAGMA journal_mode")
            .fetch_one(&mut connection)
            .await
            .context(
                "cannot access source database exclusively; close its SQLite clients on all hosts",
            )?;
            if mode == "delete" {
                ensure!(
                    !regular_file_exists(&replacement.replacement_path())?,
                    "unfinished backup exists"
                );
                return Ok(false);
            }
            ensure!(
                mode == "wal",
                "source database must be in WAL mode (found {mode})"
            );
            // BEGIN EXCLUSIVE alone blocks only WAL writers. EXCLUSIVE locking_mode also blocks readers.
            // The mode keeps the database lock after ROLLBACK.
            sqlx::raw_sql("BEGIN EXCLUSIVE; ROLLBACK;")
            .execute(&mut connection)
            .await
            .context(
                "cannot lock source database exclusively; close its SQLite clients on all hosts",
            )?;

            // A DELETE no-op must not open raw descriptors: closing them could release
            // POSIX locks held by another pool in this process.
            source_files.push(File::open(source)?);
            #[cfg(unix)]
            access::require_private_database(&source_files[0])?;
            // The private directory protects WAL contents even on a shared /tmp.
            let staging = tempfile::Builder::new()
                .prefix("codex-nfs-migration-")
                .tempdir_in(staging_dir)
                .context("cannot create local staging directory")?;
            let staged_database = staging.path().join("database.sqlite");

            let mut wal_path = source.as_os_str().to_owned();
            wal_path.push("-wal");
            match File::open(wal_path) {
                Ok(file) => source_files.push(file),
                Err(error) if error.kind() == ErrorKind::NotFound => {}
                Err(error) => return Err(error).context("cannot open source WAL"),
            }
            for (file, name) in source_files
                .iter_mut()
                .zip(["database.sqlite", "database.sqlite-wal"])
            {
                let mut target = File::create(staging.path().join(name))?;
                copy_buffered(file, &mut target).context("cannot stage source database and WAL")?;
            }
            let mut copy = SqliteConnection::connect_with(
                &SqliteConnectOptions::new()
                    .filename(&staged_database)
                    .locking_mode(SqliteLockingMode::Exclusive)
                    .journal_mode(SqliteJournalMode::Delete)
                    .synchronous(SqliteSynchronous::Full)
                    .disable_statement_logging(),
            )
            .await
            .context("cannot convert staged database to DELETE")?;
            let validated = validate_delete_database(&mut copy)
                .await
                .context("staged database failed validation");
            let closed = copy.close().await;
            validated?;
            closed?;

            replacement.prepare_backup_directory()?;
            let mut temporary = tempfile::NamedTempFile::new_in(&replacement.backup_directory)?;
            copy_buffered(&mut File::open(&staged_database)?, temporary.as_file_mut())
                .context("cannot write replacement copy")?;
            // Use the open handle. Closing another source descriptor can release SQLite's locks.
            temporary
                .as_file()
                .set_permissions(source_files[0].metadata()?.permissions())?;
            #[cfg(unix)]
            access::require_private_database(temporary.as_file())?;
            temporary.as_file().sync_all()?;
            temporary
                .into_temp_path()
                .persist_noclobber(replacement.replacement_path())
                .context("cannot save replacement copy")?;
            File::open(&replacement.backup_directory)?.sync_all()?;
            for file in &source_files {
                file.sync_all()?;
            }
            replacement.move_original_to_backup()?;
            Ok(true)
        }
        .await;
        let closed = connection.close().await;
        drop(source_files);
        let changed = result?;
        closed.context("cannot close source database. Keep clients stopped")?;
        if changed {
            replacement.install()?;
            return Ok(NfsDbMigrationOutcome::Migrated {
                backup_directory: replacement.backup_directory,
            });
        }
        Ok(NfsDbMigrationOutcome::AlreadyDelete)
    }
}

async fn validate_delete_database(connection: &mut SqliteConnection) -> anyhow::Result<()> {
    let mode: String = sqlx::query_scalar("PRAGMA journal_mode")
        .fetch_one(&mut *connection)
        .await?;
    ensure!(
        mode == "delete",
        "database did not enter DELETE journal mode"
    );
    let integrity: String = sqlx::query_scalar("PRAGMA integrity_check(1)")
        .fetch_one(connection)
        .await
        .context("database failed integrity check")?;
    ensure!(
        integrity == "ok",
        "database failed integrity check: {integrity}"
    );
    Ok(())
}

fn copy_buffered(source: &mut File, destination: &mut File) -> std::io::Result<()> {
    // Use large reads to avoid the fallback to small reads for copy_file_range on NFSv4.1.
    let mut buffer = vec![0; 1024 * 1024];
    loop {
        let bytes = match source.read(&mut buffer) {
            Err(error) if error.kind() == ErrorKind::Interrupted => continue,
            result => result?,
        };
        if bytes == 0 {
            return Ok(());
        }
        destination.write_all(&buffer[..bytes])?;
    }
}
