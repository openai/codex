use super::RuntimeDbSpec;
use anyhow::Context;
use anyhow::Result;
use log::LevelFilter;
use sqlx::ConnectOptions;
use sqlx::Row;
use sqlx::SqlitePool;
use sqlx::migrate::Migrator;
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::sqlite::SqlitePoolOptions;
use std::borrow::Cow;
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;
use tracing::warn;

mod lost_and_found;
mod recover_api;

const SQLITE_CORRUPT: i32 = 11;
const SQLITE_NOTADB: i32 = 26;

#[derive(Debug)]
struct RecoveryPaths {
    recovered_path: PathBuf,
    backup_paths: Vec<PathBuf>,
}

pub(super) fn is_malformed_sqlite_error(err: &anyhow::Error) -> bool {
    // Prefer SQLite result codes, but keep a message fallback for migration
    // wrappers that stringify the underlying database error.
    for cause in err.chain() {
        if let Some(sqlx_err) = cause.downcast_ref::<sqlx::Error>()
            && sqlx_error_is_malformed(sqlx_err)
        {
            return true;
        }
    }

    err.chain()
        .map(ToString::to_string)
        .any(|message| error_message_is_malformed(message.as_str()))
}

pub(super) async fn recover_database(
    path: &Path,
    spec: RuntimeDbSpec,
    migrator: &Migrator,
    original_error: &anyhow::Error,
) -> Result<()> {
    let recovery = prepare_recovery_paths(path).await.with_context(|| {
        format!(
            "failed to prepare automatic recovery for {} at {}",
            spec.label,
            path.display()
        )
    })?;
    warn!(
        "{} at {} appears malformed ({original_error}); attempting automatic recovery after backing up to {}",
        spec.label,
        path.display(),
        format_backup_paths(&recovery.backup_paths)
    );
    print_status(recovery_started_status(
        path,
        spec,
        recovery.backup_paths.as_slice(),
    ));

    match run_recovery(path, recovery.recovered_path.as_path(), migrator).await {
        Ok(()) => {
            if let Err(err) =
                replace_with_recovered_database(path, recovery.recovered_path.as_path()).await
            {
                print_status(recovery_failed_status(
                    path,
                    spec,
                    recovery.backup_paths.as_slice(),
                ));
                return Err(err).with_context(|| {
                    format!(
                        "automatic recovery rebuilt {} at {} but failed to replace the original database; backup files remain at {}",
                        spec.label,
                        path.display(),
                        format_backup_paths(&recovery.backup_paths)
                    )
                });
            }
            warn!(
                "automatically recovered {} at {} with the SQLite recovery API",
                spec.label,
                path.display()
            );
            print_status(recovery_completed_status(path, spec));
            Ok(())
        }
        Err(err) => {
            let _ = tokio::fs::remove_file(recovery.recovered_path.as_path()).await;
            print_status(recovery_failed_status(
                path,
                spec,
                recovery.backup_paths.as_slice(),
            ));
            Err(err).with_context(|| {
                format!(
                    "automatic recovery failed for {} at {}; backup files remain at {}",
                    spec.label,
                    path.display(),
                    format_backup_paths(&recovery.backup_paths)
                )
            })
        }
    }
}

fn print_status(message: String) {
    // Keep startup recovery status on stderr so stdout remains available for
    // command output and app-server JSON-RPC transports.
    eprintln!("{message}");
}

fn sqlx_error_is_malformed(err: &sqlx::Error) -> bool {
    match err {
        sqlx::Error::Database(database_error) => {
            let code = database_error
                .code()
                .unwrap_or(Cow::Borrowed("none"))
                .to_string();
            sqlite_code_is_malformed(code.as_str())
                || error_message_is_malformed(database_error.message())
        }
        _ => error_message_is_malformed(err.to_string().as_str()),
    }
}

fn sqlite_code_is_malformed(code: &str) -> bool {
    let primary_code = code.parse::<i32>().ok().map(|code| code & 0xff);
    matches!(primary_code, Some(SQLITE_CORRUPT | SQLITE_NOTADB))
}

fn error_message_is_malformed(message: &str) -> bool {
    let message = message.to_ascii_lowercase();
    message.contains("database disk image is malformed")
        || message.contains("file is not a database")
        || message.contains("database schema is malformed")
        || message.contains("database is corrupt")
        || (message.contains("database") && message.contains("malformed"))
}

async fn prepare_recovery_paths(path: &Path) -> Result<RecoveryPaths> {
    if !tokio::fs::try_exists(path).await? {
        anyhow::bail!("database file does not exist");
    }

    let suffix = recovery_suffix();
    let backup_paths = backup_sqlite_files(path, suffix.as_str()).await?;
    if backup_paths.is_empty() {
        anyhow::bail!("no database files were available to back up");
    }
    let recovered_path = unique_sibling_path(path, suffix.as_str(), "recovered").await?;
    Ok(RecoveryPaths {
        recovered_path,
        backup_paths,
    })
}

async fn backup_sqlite_files(path: &Path, suffix: &str) -> Result<Vec<PathBuf>> {
    let mut backups = Vec::new();
    for sqlite_path in sqlite_paths(path) {
        if tokio::fs::try_exists(sqlite_path.as_path()).await? {
            let backup_path = unique_sibling_path(sqlite_path.as_path(), suffix, "bak").await?;
            tokio::fs::copy(sqlite_path.as_path(), backup_path.as_path()).await?;
            backups.push(backup_path);
        }
    }
    Ok(backups)
}

async fn unique_sibling_path(path: &Path, suffix: &str, extension: &str) -> Result<PathBuf> {
    let file_name = path.file_name().ok_or_else(|| {
        anyhow::anyhow!("cannot create a recovery file name for {}", path.display())
    })?;
    let mut sequence = 0;
    loop {
        let mut candidate = file_name.to_os_string();
        candidate.push(format!(".{suffix}.{sequence}.{extension}"));
        let candidate = path.with_file_name(candidate);
        if !tokio::fs::try_exists(candidate.as_path()).await? {
            return Ok(candidate);
        }
        sequence += 1;
    }
}

async fn run_recovery(path: &Path, recovered_path: &Path, migrator: &Migrator) -> Result<()> {
    let path = path.to_path_buf();
    let recovered_path = recovered_path.to_path_buf();
    let recovered_path_for_task = recovered_path.clone();
    tokio::task::spawn_blocking(move || {
        recover_api::recover(path.as_path(), recovered_path_for_task.as_path())
    })
    .await
    .context("sqlite recovery task panicked")??;

    let pool = open_recovered_pool(recovered_path.as_path()).await?;
    lost_and_found::rebuild_from_recovered_schema_if_needed(&pool).await?;
    assert_recovered_schema(&pool).await?;
    assert_integrity_ok(&pool).await?;
    match migrator.run(&pool).await {
        Ok(()) => {
            assert_integrity_ok(&pool).await?;
            pool.close().await;
        }
        Err(err) => {
            pool.close().await;
            // Recovery can restore user tables while losing SQLx's bookkeeping
            // table. Normalize through a fresh migrated DB before giving up.
            rebuild_recovered_database(recovered_path.as_path(), migrator)
                .await
                .with_context(|| {
                    format!("failed to normalize recovered database after migration failure: {err}")
                })?;
        }
    }
    Ok(())
}

async fn open_recovered_pool(path: &Path) -> Result<SqlitePool> {
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(false)
        .busy_timeout(Duration::from_secs(5))
        .log_statements(LevelFilter::Off);
    Ok(SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await?)
}

async fn open_migrated_pool(path: &Path, migrator: &Migrator) -> Result<SqlitePool> {
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true)
        .busy_timeout(Duration::from_secs(5))
        .log_statements(LevelFilter::Off);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await?;
    migrator
        .run(&pool)
        .await
        .context("failed to migrate fresh database for recovered data")?;
    Ok(pool)
}

async fn rebuild_recovered_database(recovered_path: &Path, migrator: &Migrator) -> Result<()> {
    let normalized_path =
        unique_sibling_path(recovered_path, "codex-recovery-normalized", "sqlite").await?;
    let pool = open_migrated_pool(normalized_path.as_path(), migrator).await?;
    let rebuild_result = async {
        copy_recovered_tables(&pool, recovered_path).await?;
        assert_recovered_schema(&pool).await?;
        assert_integrity_ok(&pool).await?;
        Ok::<(), anyhow::Error>(())
    }
    .await;
    pool.close().await;

    if let Err(err) = rebuild_result {
        let _ = tokio::fs::remove_file(normalized_path.as_path()).await;
        return Err(err);
    }

    tokio::fs::remove_file(recovered_path).await?;
    tokio::fs::rename(normalized_path, recovered_path).await?;
    Ok(())
}

async fn copy_recovered_tables(pool: &SqlitePool, recovered_path: &Path) -> Result<()> {
    let recovered_path = recovered_path.to_str().with_context(|| {
        format!(
            "recovered path is not valid UTF-8: {}",
            recovered_path.display()
        )
    })?;
    sqlx::query("ATTACH DATABASE ? AS recovered")
        .bind(recovered_path)
        .execute(pool)
        .await?;
    sqlx::query("PRAGMA foreign_keys = OFF")
        .execute(pool)
        .await?;

    let copy_result = async {
        let destination_tables = user_tables(pool, "main").await?;
        let source_tables = user_tables(pool, "recovered").await?;
        let mut copied_table_count = 0;
        for table in source_tables {
            if destination_tables.contains(&table) {
                // Copy only columns that survived recovery and still exist in
                // the current schema. Missing new columns rely on migration
                // defaults.
                if copy_current_schema_table(pool, table.as_str()).await? {
                    copied_table_count += 1;
                }
            } else {
                copy_extra_recovered_table(pool, table.as_str()).await?;
                copied_table_count += 1;
            }
        }
        if copied_table_count == 0 {
            anyhow::bail!("recovered database did not contain any current schema tables");
        }
        Ok::<(), anyhow::Error>(())
    }
    .await;

    let detach_result = sqlx::query("DETACH DATABASE recovered").execute(pool).await;
    copy_result?;
    detach_result?;
    Ok(())
}

async fn user_tables(pool: &SqlitePool, schema: &str) -> Result<BTreeSet<String>> {
    let sql = format!(
        r#"
SELECT name
FROM {}.sqlite_schema
WHERE type = 'table'
  AND name NOT LIKE 'sqlite_%'
  AND name != '_sqlx_migrations'
ORDER BY name
        "#,
        quote_identifier(schema)
    );
    let rows = sqlx::query_scalar::<_, String>(sql.as_str())
        .fetch_all(pool)
        .await?;
    Ok(rows.into_iter().collect())
}

async fn copy_current_schema_table(pool: &SqlitePool, table: &str) -> Result<bool> {
    let destination_columns = table_columns(pool, "main", table).await?;
    let source_columns = table_columns(pool, "recovered", table).await?;
    let source_columns = source_columns.into_iter().collect::<BTreeSet<_>>();
    let columns = destination_columns
        .into_iter()
        .filter(|column| source_columns.contains(column))
        .collect::<Vec<_>>();
    if columns.is_empty() {
        return Ok(false);
    }

    let column_list = columns
        .iter()
        .map(|column| quote_identifier(column))
        .collect::<Vec<_>>()
        .join(", ");
    let table_name = quote_identifier(table);
    let sql = format!(
        "INSERT OR REPLACE INTO main.{table_name} ({column_list}) SELECT {column_list} FROM recovered.{table_name}"
    );
    sqlx::query(sql.as_str()).execute(pool).await?;
    Ok(true)
}

async fn copy_extra_recovered_table(pool: &SqlitePool, table: &str) -> Result<()> {
    let table_name = quote_identifier(table);
    let sql = format!("CREATE TABLE main.{table_name} AS SELECT * FROM recovered.{table_name}");
    sqlx::query(sql.as_str()).execute(pool).await?;
    Ok(())
}

async fn table_columns(pool: &SqlitePool, schema: &str, table: &str) -> Result<Vec<String>> {
    let sql = format!(
        "SELECT name FROM {}.pragma_table_xinfo(?) WHERE hidden = 0 ORDER BY cid",
        quote_identifier(schema)
    );
    Ok(sqlx::query_scalar::<_, String>(sql.as_str())
        .bind(table)
        .fetch_all(pool)
        .await?)
}

fn quote_identifier(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}

async fn assert_recovered_schema(pool: &SqlitePool) -> Result<()> {
    // A non-SQLite file can produce a valid empty database through recovery;
    // accepting that would silently turn a damaged database into data loss.
    let row = sqlx::query(
        r#"
SELECT COUNT(*) AS table_count
FROM sqlite_schema
WHERE type = 'table'
  AND name NOT LIKE 'sqlite_%'
  AND name != '_sqlx_migrations'
            "#,
    )
    .fetch_one(pool)
    .await?;
    let table_count: i64 = row.try_get("table_count")?;
    if table_count == 0 {
        anyhow::bail!("SQLite recovery did not recover any user tables");
    }
    Ok(())
}

async fn assert_integrity_ok(pool: &SqlitePool) -> Result<()> {
    let rows = sqlx::query_scalar::<_, String>("PRAGMA integrity_check")
        .fetch_all(pool)
        .await?;
    if !rows.iter().all(|row| row == "ok") {
        anyhow::bail!(
            "recovered database failed integrity_check: {}",
            rows.join("; ")
        );
    }
    Ok(())
}

async fn replace_with_recovered_database(path: &Path, recovered_path: &Path) -> Result<()> {
    // Stale WAL files belong to the damaged database and must not be replayed
    // against the recovered replacement.
    for sidecar in sqlite_sidecar_paths(path) {
        match tokio::fs::remove_file(sidecar.as_path()).await {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => return Err(err.into()),
        }
    }

    #[cfg(windows)]
    match tokio::fs::remove_file(path).await {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => return Err(err.into()),
    }

    tokio::fs::rename(recovered_path, path).await?;
    Ok(())
}

fn sqlite_paths(path: &Path) -> Vec<PathBuf> {
    let mut paths = vec![path.to_path_buf()];
    paths.extend(sqlite_sidecar_paths(path));
    paths
}

fn sqlite_sidecar_paths(path: &Path) -> Vec<PathBuf> {
    ["-wal", "-shm"]
        .into_iter()
        .map(|suffix| {
            let mut sidecar = OsString::from(path.as_os_str());
            sidecar.push(suffix);
            PathBuf::from(sidecar)
        })
        .collect()
}

fn recovery_suffix() -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs());
    format!("codex-recovery-{timestamp}")
}

fn format_backup_paths(paths: &[PathBuf]) -> String {
    paths
        .iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

fn recovery_started_status(path: &Path, spec: RuntimeDbSpec, backup_paths: &[PathBuf]) -> String {
    format!(
        "Codex detected a malformed {} at {}. Please wait while Codex attempts to recover it automatically. Backup files: {}",
        spec.label,
        path.display(),
        format_backup_paths(backup_paths)
    )
}

fn recovery_completed_status(path: &Path, spec: RuntimeDbSpec) -> String {
    format!(
        "Codex successfully recovered {} at {}.",
        spec.label,
        path.display()
    )
}

fn recovery_failed_status(path: &Path, spec: RuntimeDbSpec, backup_paths: &[PathBuf]) -> String {
    format!(
        "Codex could not automatically recover {} at {}. Backup files remain at {}.",
        spec.label,
        path.display(),
        format_backup_paths(backup_paths)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use sqlx::migrate::Migrator;
    use std::fs::OpenOptions;
    use std::io::Read;
    use std::io::Seek;
    use std::io::SeekFrom;
    use std::io::Write;

    #[test]
    fn detects_malformed_sqlite_messages() {
        let err = anyhow::anyhow!("database disk image is malformed");

        assert!(is_malformed_sqlite_error(&err));
    }

    #[test]
    fn recovery_status_messages_include_path_and_backup() {
        let db_path = Path::new("state.sqlite");
        let backup_path = PathBuf::from("state.sqlite.codex-recovery-1.0.bak");

        assert_eq!(
            recovery_started_status(
                db_path,
                super::super::STATE_DB,
                std::slice::from_ref(&backup_path)
            ),
            "Codex detected a malformed state DB at state.sqlite. Please wait while Codex attempts to recover it automatically. Backup files: state.sqlite.codex-recovery-1.0.bak"
        );
        assert_eq!(
            recovery_completed_status(db_path, super::super::STATE_DB),
            "Codex successfully recovered state DB at state.sqlite."
        );
        assert_eq!(
            recovery_failed_status(db_path, super::super::STATE_DB, &[backup_path]),
            "Codex could not automatically recover state DB at state.sqlite. Backup files remain at state.sqlite.codex-recovery-1.0.bak."
        );
    }

    #[tokio::test]
    async fn recovery_preserves_backup_and_replaces_malformed_database() -> Result<()> {
        let temp_dir = super::super::test_support::unique_temp_dir();
        tokio::fs::create_dir_all(temp_dir.as_path()).await?;
        let db_path = temp_dir.join("sample.sqlite");
        create_sample_db(db_path.as_path()).await?;
        corrupt_first_table_page(db_path.as_path())?;

        let err = anyhow::anyhow!("database disk image is malformed");
        recover_database(
            db_path.as_path(),
            super::super::STATE_DB,
            &Migrator::DEFAULT,
            &err,
        )
        .await?;

        let rows = super::super::sqlite_integrity_check(db_path.as_path()).await?;
        assert_eq!(rows, vec!["ok".to_string()]);
        let backup_count = std::fs::read_dir(temp_dir.as_path())?
            .filter_map(std::result::Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .contains(".codex-recovery-")
            })
            .filter(|entry| entry.file_name().to_string_lossy().ends_with(".bak"))
            .count();
        assert_eq!(backup_count, 1);
        let _ = tokio::fs::remove_dir_all(temp_dir.as_path()).await;
        Ok(())
    }

    #[tokio::test]
    async fn recovery_normalizes_database_when_migration_metadata_is_lost() -> Result<()> {
        let temp_dir = super::super::test_support::unique_temp_dir();
        tokio::fs::create_dir_all(temp_dir.as_path()).await?;
        let db_path = temp_dir.join(super::super::STATE_DB.filename);
        let migrator = crate::migrations::runtime_state_migrator();
        let pool = open_migrated_pool(db_path.as_path(), &migrator).await?;
        let thread_id = "00000000-0000-0000-0000-000000000456";
        sqlx::query(
            r#"
INSERT INTO threads (
    id,
    rollout_path,
    created_at,
    updated_at,
    source,
    model_provider,
    cwd,
    title,
    sandbox_policy,
    approval_mode
) VALUES (?, ?, 1, 1, 'cli', 'test-provider', ?, 'survived recovery', 'read-only', 'on-request')
            "#,
        )
        .bind(thread_id)
        .bind(temp_dir.join("session.jsonl").display().to_string())
        .bind(temp_dir.as_path().display().to_string())
        .execute(&pool)
        .await?;
        sqlx::query(
            "CREATE TABLE extra_recovered_data (id INTEGER PRIMARY KEY, value TEXT NOT NULL)",
        )
        .execute(&pool)
        .await?;
        sqlx::query("INSERT INTO extra_recovered_data (value) VALUES ('preserved')")
            .execute(&pool)
            .await?;
        let page_size: i64 = sqlx::query_scalar("PRAGMA page_size")
            .fetch_one(&pool)
            .await?;
        let migration_root_page: i64 = sqlx::query_scalar(
            "SELECT rootpage FROM sqlite_schema WHERE name = '_sqlx_migrations'",
        )
        .fetch_one(&pool)
        .await?;
        pool.close().await;

        corrupt_page(
            db_path.as_path(),
            page_size.try_into()?,
            migration_root_page.try_into()?,
        )?;

        let err = anyhow::anyhow!("database disk image is malformed");
        recover_database(db_path.as_path(), super::super::STATE_DB, &migrator, &err).await?;

        let pool = open_recovered_pool(db_path.as_path()).await?;
        let title: String = sqlx::query_scalar("SELECT title FROM threads WHERE id = ?")
            .bind(thread_id)
            .fetch_one(&pool)
            .await?;
        let migration_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations")
            .fetch_one(&pool)
            .await?;
        let extra_value: String =
            sqlx::query_scalar("SELECT value FROM extra_recovered_data WHERE id = 1")
                .fetch_one(&pool)
                .await?;
        pool.close().await;

        assert_eq!(title, "survived recovery");
        assert_eq!(migration_count, migrator.migrations.len() as i64);
        assert_eq!(extra_value, "preserved");
        let _ = tokio::fs::remove_dir_all(temp_dir.as_path()).await;
        Ok(())
    }

    #[tokio::test]
    async fn recovery_rebuilds_when_header_and_schema_root_are_lost() -> Result<()> {
        let temp_dir = super::super::test_support::unique_temp_dir();
        tokio::fs::create_dir_all(temp_dir.as_path()).await?;
        let db_path = temp_dir.join(super::super::STATE_DB.filename);
        let migrator = crate::migrations::runtime_state_migrator();
        let pool = open_migrated_pool(db_path.as_path(), &migrator).await?;
        let thread_id = "00000000-0000-0000-0000-000000000789";
        sqlx::query(
            r#"
INSERT INTO threads (
    id,
    rollout_path,
    created_at,
    updated_at,
    source,
    model_provider,
    cwd,
    title,
    sandbox_policy,
    approval_mode
) VALUES (?, ?, 1, 1, 'cli', 'test-provider', ?, 'schema root lost', 'read-only', 'on-request')
            "#,
        )
        .bind(thread_id)
        .bind(temp_dir.join("session.jsonl").display().to_string())
        .bind(temp_dir.as_path().display().to_string())
        .execute(&pool)
        .await?;
        sqlx::query(
            r#"
INSERT INTO thread_dynamic_tools (
    thread_id,
    position,
    name,
    description,
    input_schema
) VALUES (?, 0, 'shell', 'run a command', '{"type":"object"}')
            "#,
        )
        .bind(thread_id)
        .execute(&pool)
        .await?;
        let page_size: i64 = sqlx::query_scalar("PRAGMA page_size")
            .fetch_one(&pool)
            .await?;
        let threads_root_page: i64 =
            sqlx::query_scalar("SELECT rootpage FROM sqlite_schema WHERE name = 'threads'")
                .fetch_one(&pool)
                .await?;
        pool.close().await;

        overwrite_page_with_page(
            db_path.as_path(),
            page_size.try_into()?,
            threads_root_page.try_into()?,
            /*destination_page*/ 1,
        )?;

        let err = anyhow::anyhow!("file is not a database");
        recover_database(db_path.as_path(), super::super::STATE_DB, &migrator, &err).await?;

        let pool = open_recovered_pool(db_path.as_path()).await?;
        let title: String = sqlx::query_scalar("SELECT title FROM threads WHERE id = ?")
            .bind(thread_id)
            .fetch_one(&pool)
            .await?;
        let tool_schema: String =
            sqlx::query_scalar("SELECT input_schema FROM thread_dynamic_tools WHERE thread_id = ?")
                .bind(thread_id)
                .fetch_one(&pool)
                .await?;
        let migration_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations")
            .fetch_one(&pool)
            .await?;
        pool.close().await;

        assert_eq!(title, "schema root lost");
        assert_eq!(tool_schema, r#"{"type":"object"}"#);
        assert_eq!(migration_count, migrator.migrations.len() as i64);
        let _ = tokio::fs::remove_dir_all(temp_dir.as_path()).await;
        Ok(())
    }

    #[tokio::test]
    async fn recovery_rejects_output_without_user_tables() -> Result<()> {
        let temp_dir = super::super::test_support::unique_temp_dir();
        tokio::fs::create_dir_all(temp_dir.as_path()).await?;
        let db_path = temp_dir.join("not-a-db.sqlite");
        tokio::fs::write(db_path.as_path(), b"not sqlite").await?;
        let err = anyhow::anyhow!("file is not a database");

        let recover_err = recover_database(
            db_path.as_path(),
            super::super::STATE_DB,
            &Migrator::DEFAULT,
            &err,
        )
        .await
        .expect_err("recovery without schema should fail");

        assert!(
            recover_err
                .to_string()
                .contains("automatic recovery failed"),
            "unexpected error: {recover_err}"
        );
        assert!(tokio::fs::try_exists(db_path.as_path()).await?);
        let _ = tokio::fs::remove_dir_all(temp_dir.as_path()).await;
        Ok(())
    }

    async fn create_sample_db(path: &Path) -> Result<()> {
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .log_statements(LevelFilter::Off);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await?;
        sqlx::query("CREATE TABLE sample (id INTEGER PRIMARY KEY, value TEXT NOT NULL)")
            .execute(&pool)
            .await?;
        sqlx::query("INSERT INTO sample (value) VALUES ('one'), ('two')")
            .execute(&pool)
            .await?;
        pool.close().await;
        Ok(())
    }

    fn corrupt_first_table_page(path: &Path) -> Result<()> {
        let mut bytes = std::fs::read(path)?;
        if bytes.len() <= 4096 {
            anyhow::bail!("sample database was smaller than two pages");
        }
        bytes[4096] = 0;
        std::fs::write(path, bytes)?;
        Ok(())
    }

    fn corrupt_page(path: &Path, page_size: u64, page_number: u64) -> Result<()> {
        let offset = page_size
            .checked_mul(page_number.saturating_sub(1))
            .context("corrupt page offset overflowed")?;
        let mut file = OpenOptions::new().write(true).open(path)?;
        file.seek(SeekFrom::Start(offset))?;
        file.write_all(&[0; 16])?;
        Ok(())
    }

    fn overwrite_page_with_page(
        path: &Path,
        page_size: u64,
        source_page: u64,
        destination_page: u64,
    ) -> Result<()> {
        let mut page = vec![0; page_size.try_into()?];
        let mut file = OpenOptions::new().read(true).write(true).open(path)?;
        let source_offset = page_size
            .checked_mul(source_page.saturating_sub(1))
            .context("source page offset overflowed")?;
        file.seek(SeekFrom::Start(source_offset))?;
        file.read_exact(page.as_mut_slice())?;
        let destination_offset = page_size
            .checked_mul(destination_page.saturating_sub(1))
            .context("destination page offset overflowed")?;
        file.seek(SeekFrom::Start(destination_offset))?;
        file.write_all(page.as_slice())?;
        Ok(())
    }
}
