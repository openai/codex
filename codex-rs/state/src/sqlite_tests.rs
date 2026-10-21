//! Regression coverage for SQLite initialization, cached filesystem detection, and competing locks.

use super::SqliteConfig;
use codex_utils_absolute_path::test_support::PathExt;
use pretty_assertions::assert_eq;
use sqlx::sqlite::SqliteAutoVacuum;
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::sqlite::SqliteJournalMode;
use sqlx::sqlite::SqlitePoolOptions;

#[tokio::test]
async fn configured_log_directory_is_initialized_on_use_and_retries_after_failure()
-> anyhow::Result<()> {
    let sqlite_home = crate::runtime::test_support::unique_temp_dir();
    tokio::fs::create_dir_all(&sqlite_home).await?;
    let _cleanup = scopeguard::guard(sqlite_home.clone(), |sqlite_home| {
        let _ = std::fs::remove_dir_all(sqlite_home);
    });
    let logs_home = sqlite_home.join("logs");
    tokio::fs::write(&logs_home, "not a directory").await?;
    let sqlite = SqliteConfig::new_for_testing(sqlite_home.as_path().abs());
    let fallback_logs_path = sqlite.logs_db_path();
    let sqlite = sqlite.with_local_logs_home(logs_home.clone());

    // Opening unrelated state must not require the configured logs directory.
    let pool = sqlite.open_read_write_pool(&sqlite.state_db_path()).await?;
    pool.close().await;
    assert!(
        crate::StateRuntime::init(sqlite.clone(), "openai".to_string())
            .await
            .is_err()
    );
    assert!(!fallback_logs_path.exists());

    // A failed initialization must remain retryable without replacing the config.
    tokio::fs::remove_file(&logs_home).await?;
    let runtime = crate::StateRuntime::init(sqlite.clone(), "openai".to_string()).await?;
    assert!(sqlite.logs_db_path().is_file());
    assert!(!fallback_logs_path.exists());
    runtime.close().await;
    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn configured_log_directory_lock_is_shared_by_reader_and_writer_config_clones()
-> anyhow::Result<()> {
    let sqlite_home = crate::runtime::test_support::unique_temp_dir();
    let logs_home = sqlite_home.join("logs");
    tokio::fs::create_dir_all(&logs_home).await?;
    let _cleanup = scopeguard::guard(sqlite_home.clone(), |sqlite_home| {
        let _ = std::fs::remove_dir_all(sqlite_home);
    });
    let initial = SqliteConfig::new_for_testing(logs_home.as_path().abs());
    let logs_path = initial.logs_db_path();
    initial
        .open_read_write_pool(&logs_path)
        .await?
        .close()
        .await;

    for read_only in [false, true] {
        let sqlite = SqliteConfig::new_for_testing(sqlite_home.as_path().abs())
            .with_local_logs_home(logs_home.clone());
        let clone = sqlite.clone();
        let cleanup_lock = std::fs::File::open(&logs_home)?;
        cleanup_lock.try_lock()?;
        cleanup_lock.unlock()?;

        let pool = if read_only {
            sqlite
                .open_read_only_pool(&logs_path, /*busy_timeout*/ None)
                .await?
        } else {
            sqlite.open_read_write_pool(&logs_path).await?
        };
        assert!(matches!(
            cleanup_lock.try_lock(),
            Err(std::fs::TryLockError::WouldBlock)
        ));
        drop(sqlite);
        assert!(matches!(
            cleanup_lock.try_lock(),
            Err(std::fs::TryLockError::WouldBlock)
        ));

        pool.close().await;
        drop(clone);
        cleanup_lock.try_lock()?;
    }
    Ok(())
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn sqlite_home_filesystem_probe_is_shared_by_clones_until_config_replacement()
-> anyhow::Result<()> {
    let sqlite_home = crate::runtime::test_support::unique_temp_dir();
    tokio::fs::create_dir_all(&sqlite_home).await?;
    let _cleanup = scopeguard::guard(sqlite_home.clone(), |sqlite_home| {
        let _ = std::fs::remove_dir_all(sqlite_home);
    });
    let sqlite = SqliteConfig::new_for_testing(sqlite_home.as_path().abs());
    let clone = sqlite.clone();
    let is_nfs = sqlite.is_nfs().await?;
    tokio::fs::remove_dir(&sqlite_home).await?;
    // This clone has never queried the home. It must reuse the original's probe,
    // because a fresh filesystem lookup would fail now that the home is absent.
    assert_eq!(clone.is_nfs().await?, is_nfs);

    let fresh = SqliteConfig::new_for_testing(sqlite_home.as_path().abs());
    assert_eq!(
        fresh
            .is_nfs()
            .await
            .unwrap_err()
            .downcast_ref::<std::io::Error>()
            .unwrap()
            .kind(),
        std::io::ErrorKind::NotFound
    );
    Ok(())
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn failed_sqlite_home_filesystem_probe_recovers_after_home_creation() -> anyhow::Result<()> {
    let sqlite_home = crate::runtime::test_support::unique_temp_dir();
    let _cleanup = scopeguard::guard(sqlite_home.clone(), |sqlite_home| {
        let _ = std::fs::remove_dir_all(sqlite_home);
    });
    let sqlite = SqliteConfig::new_for_testing(sqlite_home.as_path().abs());
    assert_eq!(
        sqlite
            .is_nfs()
            .await
            .unwrap_err()
            .downcast_ref::<std::io::Error>()
            .unwrap()
            .kind(),
        std::io::ErrorKind::NotFound
    );
    tokio::fs::create_dir_all(&sqlite_home).await?;
    sqlite.is_nfs().await?;
    Ok(())
}

#[tokio::test]
async fn open_read_write_pool_initializes_fresh_database_settings() -> anyhow::Result<()> {
    for interrupted in [false, true] {
        let sqlite_home = crate::runtime::test_support::unique_temp_dir();
        tokio::fs::create_dir_all(&sqlite_home).await?;
        let _cleanup = scopeguard::guard(sqlite_home.clone(), |sqlite_home| {
            let _ = std::fs::remove_dir_all(sqlite_home);
        });
        let sqlite = SqliteConfig::new_for_testing(sqlite_home.as_path().abs());
        let path = sqlite.logs_db_path();
        if interrupted {
            // Auto-vacuum writes a header before WAL setup.
            // Startup must initialize the database even if the file is not empty.
            let partial = SqlitePoolOptions::new()
                .max_connections(/*max*/ 1)
                .connect_with(
                    SqliteConnectOptions::new()
                        .filename(&path)
                        .create_if_missing(/*create*/ true)
                        .auto_vacuum(SqliteAutoVacuum::Incremental),
                )
                .await?;
            partial.close().await;
            assert!(tokio::fs::metadata(&path).await?.len() > 0);
        }
        let pool = sqlite.open_read_write_pool(&path).await?;
        let journal_mode = sqlx::query_scalar::<_, String>("PRAGMA journal_mode")
            .fetch_one(&pool)
            .await?;
        let auto_vacuum = sqlx::query_scalar::<_, i64>("PRAGMA auto_vacuum")
            .fetch_one(&pool)
            .await?;
        let synchronous = sqlx::query_scalar::<_, i64>("PRAGMA synchronous")
            .fetch_one(&pool)
            .await?;
        assert_eq!(
            (journal_mode, auto_vacuum, synchronous),
            ("wal".to_string(), SqliteAutoVacuum::Incremental as i64, 1)
        );
        pool.close().await;
    }
    Ok(())
}

#[tokio::test]
async fn open_read_write_pool_preserves_existing_settings_under_write_lock() -> anyhow::Result<()> {
    let sqlite_home = crate::runtime::test_support::unique_temp_dir();
    tokio::fs::create_dir_all(&sqlite_home).await?;
    let _cleanup = scopeguard::guard(sqlite_home.clone(), |sqlite_home| {
        let _ = std::fs::remove_dir_all(sqlite_home);
    });
    let sqlite = SqliteConfig::new_for_testing(sqlite_home.as_path().abs());
    let database_path = sqlite.logs_db_path();
    let existing_pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(&database_path)
                .create_if_missing(true)
                .auto_vacuum(SqliteAutoVacuum::Full)
                .journal_mode(SqliteJournalMode::Wal),
        )
        .await?;
    sqlx::query("CREATE TABLE existing (id INTEGER PRIMARY KEY)")
        .execute(&existing_pool)
        .await?;

    let mut lock_holder = existing_pool.acquire().await?;
    sqlx::query("BEGIN IMMEDIATE")
        .execute(&mut *lock_holder)
        .await?;

    let pool = sqlite.open_read_write_pool(&database_path).await?;
    let auto_vacuum = sqlx::query_scalar::<_, i64>("PRAGMA auto_vacuum")
        .fetch_one(&pool)
        .await?;

    assert_eq!(auto_vacuum, SqliteAutoVacuum::Full as i64);
    sqlx::query("ROLLBACK").execute(&mut *lock_holder).await?;
    drop(lock_holder);
    pool.close().await;
    existing_pool.close().await;
    Ok(())
}

#[tokio::test]
async fn open_read_write_pool_preserves_delete_under_write_lock() -> anyhow::Result<()> {
    let sqlite_home = crate::runtime::test_support::unique_temp_dir();
    tokio::fs::create_dir_all(&sqlite_home).await?;
    let _cleanup = scopeguard::guard(sqlite_home.clone(), |sqlite_home| {
        let _ = std::fs::remove_dir_all(sqlite_home);
    });
    let sqlite = SqliteConfig::new_for_testing(sqlite_home.as_path().abs());
    let database_path = sqlite.logs_db_path();
    let existing_pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(&database_path)
                .create_if_missing(true)
                .journal_mode(SqliteJournalMode::Delete),
        )
        .await?;
    sqlx::query("CREATE TABLE existing (id INTEGER PRIMARY KEY)")
        .execute(&existing_pool)
        .await?;
    let mut lock_holder = existing_pool.acquire().await?;
    sqlx::query("BEGIN IMMEDIATE")
        .execute(&mut *lock_holder)
        .await?;

    let pool = sqlite.open_read_write_pool(&database_path).await?;
    // Keep each acquired connection open to test connections that the pool initializes later.
    let mut connections = Vec::new();
    let mut settings = Vec::new();
    for _ in 0..5 {
        let mut connection = pool.acquire().await?;
        let journal: String = sqlx::query_scalar("PRAGMA journal_mode")
            .fetch_one(&mut *connection)
            .await?;
        let synchronous: i64 = sqlx::query_scalar("PRAGMA synchronous")
            .fetch_one(&mut *connection)
            .await?;
        settings.push((journal, synchronous));
        connections.push(connection);
    }
    assert_eq!(settings, vec![("delete".to_string(), 3); 5]);
    drop(connections);
    sqlx::query("ROLLBACK").execute(&mut *lock_holder).await?;
    drop(lock_holder);
    existing_pool.close().await;

    sqlx::query("INSERT INTO existing VALUES(42)")
        .execute(&pool)
        .await?;
    pool.close().await;
    let reopened = sqlite.open_read_write_pool(&database_path).await?;
    let journal: String = sqlx::query_scalar("PRAGMA journal_mode")
        .fetch_one(&reopened)
        .await?;
    let rows: Vec<i64> = sqlx::query_scalar("SELECT id FROM existing")
        .fetch_all(&reopened)
        .await?;
    assert_eq!((journal, rows), ("delete".into(), vec![42]));
    reopened.close().await;
    Ok(())
}

#[tokio::test]
async fn open_read_write_pool_preserves_empty_database_lock_error() -> anyhow::Result<()> {
    let sqlite_home = crate::runtime::test_support::unique_temp_dir();
    tokio::fs::create_dir_all(&sqlite_home).await?;
    let _cleanup = scopeguard::guard(sqlite_home.clone(), |sqlite_home| {
        let _ = std::fs::remove_dir_all(sqlite_home);
    });
    let sqlite = SqliteConfig::new_for_testing(sqlite_home.as_path().abs());
    let database_path = sqlite.logs_db_path();
    let existing_pool = SqlitePoolOptions::new()
        .max_connections(/*max*/ 1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(&database_path)
                .create_if_missing(/*create*/ true)
                .auto_vacuum(SqliteAutoVacuum::Incremental)
                .journal_mode(SqliteJournalMode::Delete),
        )
        .await?;
    // Leave the schema empty so startup must initialize WAL.
    let mut lock_holder = existing_pool.acquire().await?;
    sqlx::query("BEGIN IMMEDIATE")
        .execute(&mut *lock_holder)
        .await?;

    let error = sqlite
        .open_read_write_pool(&database_path)
        .await
        .expect_err("WAL setup must fail while another connection holds the writer lock");
    assert_eq!(
        error
            .downcast_ref::<sqlx::Error>()
            .and_then(sqlx::Error::as_database_error)
            .and_then(sqlx::error::DatabaseError::code),
        Some("5".into())
    );
    assert!(crate::sqlite_error_detail_is_lock(&error.to_string()));

    sqlx::query("ROLLBACK").execute(&mut *lock_holder).await?;
    // A failed opener must release its locks before another writer can proceed.
    sqlx::query("BEGIN EXCLUSIVE")
        .execute(&mut *lock_holder)
        .await?;
    sqlx::query("ROLLBACK").execute(&mut *lock_holder).await?;
    drop(lock_holder);
    existing_pool.close().await;

    let pool = sqlite.open_read_write_pool(&database_path).await?;
    sqlx::query("CREATE TABLE initialized (id INTEGER PRIMARY KEY)")
        .execute(&pool)
        .await?;
    let journal: String = sqlx::query_scalar("PRAGMA journal_mode")
        .fetch_one(&pool)
        .await?;
    assert_eq!(journal, "wal");
    pool.close().await;
    Ok(())
}
