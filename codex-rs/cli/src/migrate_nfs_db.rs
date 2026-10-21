//! Convert configured databases to DELETE mode and keep their WAL originals in backup directories.
//! Require confirmation before opening database files. All clients must remain stopped.
//! Keep source descriptors open until SQLite closes. Do not checkpoint the WAL source.
//! NFS must enforce POSIX locks across clients. Local descriptor checks cannot prove this.

use anyhow::Context;
use anyhow::ensure;
use clap::Parser;
use codex_config::LoaderOverrides;
use codex_core::config::ConfigBuilder;
use codex_utils_cli::CliConfigOverrides;
use owo_colors::OwoColorize;
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

#[derive(Debug, Parser)]
#[command(
    after_help = "Stop SQLite clients on every host and keep them stopped through cutover.\nThe NFS mount must support cross-client POSIX locks (no nolock/local_lock=all).\nThis command replaces configured databases in place and retains their WAL backups."
)]
pub(crate) struct MigrateNfsDbCommand {
    /// Use a local directory with space for the database and WAL files.
    /// The default is the system temporary directory.
    #[arg(long, default_value_os_t = std::env::temp_dir())]
    staging_dir: PathBuf,
}

pub(crate) async fn run(
    command: MigrateNfsDbCommand,
    config_overrides: CliConfigOverrides,
    loader_overrides: LoaderOverrides,
) -> anyhow::Result<()> {
    ensure!(
        cfg!(unix),
        "migrate-nfs-db currently requires Unix filesystem locking and directory fsync"
    );
    let warning = "WARNING: This operation is unsafe under concurrency.";
    if supports_color::on(supports_color::Stream::Stderr).is_some() {
        eprintln!("{}", warning.red().bold());
    } else {
        eprintln!("{warning}");
    }
    eprintln!(
        "Stop all Codex instances on every host that uses these databases.\nKeep them stopped until the migration completes successfully."
    );
    ensure!(
        crate::confirm("Continue? [y/N]:")?,
        "Migration cancelled. No databases were changed."
    );
    let config = ConfigBuilder::default()
        .cli_overrides(
            config_overrides
                .parse_overrides()
                .map_err(anyhow::Error::msg)?,
        )
        .loader_overrides(loader_overrides)
        .build()
        .await?;
    // Check the merged config for an explicit log directory.
    let effective_config = config.config_layer_stack.effective_config();
    let skip_logs = effective_config.get("log_dir").is_some();
    let sqlite = config.sqlite_config();
    if !sqlite.home().try_exists()? {
        println!("No databases found.");
        return Ok(());
    }
    // Keep this file after release. Removing it lets two migrations lock different inodes.
    let migration_lock = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(sqlite.home().join(".codex-nfs-migration.lock"))?;
    migration_lock
        .try_lock()
        .context("another database migration is running")?;
    for database in sqlite.runtime_db_paths() {
        if skip_logs && database.path == sqlite.logs_db_path() {
            println!("Skipped logs database: log_dir is configured.");
            continue;
        }
        migrate(&database.path, &command.staging_dir).await.with_context(|| format!(
            "migration failed for {}. Keep all Codex instances stopped. Correct the error and rerun codex migrate-nfs-db",
            database.path.display()
        ))?;
    }
    println!("Migration completed successfully. You can restart Codex.");
    Ok(())
}

#[expect(
    clippy::disallowed_methods,
    reason = "Migration needs dedicated exclusive connections, not the runtime's WAL pool"
)]
async fn migrate(source: &Path, staging_dir: &Path) -> anyhow::Result<()> {
    let replacement = Replacement::new(source)?;
    if let Recovery::Installed = replacement.resume().await? {
        println!(
            "Migrated database: {}\nWAL backup: {}",
            source.display(),
            replacement.backup_directory.display()
        );
        return Ok(());
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
        return Ok(());
    }
    let source_file = File::open(source)?;
    #[cfg(unix)]
    access::require_private_database(&source_file)?;
    // The private directory protects WAL contents even on a shared /tmp.
    let staging = tempfile::Builder::new()
        .prefix("codex-nfs-migration-")
        .tempdir_in(staging_dir)
        .context("cannot create local staging directory")?;
    let staged_database = staging.path().join("database.sqlite");
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
    let mut source_files = vec![source_file];
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
            println!("Already in DELETE mode: {}", source.display());
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
        println!(
            "Migrated database: {}\nWAL backup: {}",
            source.display(),
            replacement.backup_directory.display()
        );
    }
    Ok(())
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
