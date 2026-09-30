//! Regression coverage for SQLite initialization under competing database locks.

use super::SqliteConfig;
use codex_utils_absolute_path::test_support::PathExt;
use pretty_assertions::assert_eq;
use sqlx::sqlite::SqliteAutoVacuum;
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::sqlite::SqliteJournalMode;
use sqlx::sqlite::SqlitePoolOptions;

#[tokio::test]
async fn open_read_write_pool_initializes_fresh_database_settings() -> anyhow::Result<()> {
    let sqlite_home = crate::runtime::test_support::unique_temp_dir();
    tokio::fs::create_dir_all(&sqlite_home).await?;
    let _cleanup = scopeguard::guard(sqlite_home.clone(), |sqlite_home| {
        let _ = std::fs::remove_dir_all(sqlite_home);
    });
    let sqlite = SqliteConfig::new_for_testing(sqlite_home.as_path().abs());
    let pool = sqlite.open_read_write_pool(&sqlite.logs_db_path()).await?;

    let journal_mode = sqlx::query_scalar::<_, String>("PRAGMA journal_mode")
        .fetch_one(&pool)
        .await?;
    let auto_vacuum = sqlx::query_scalar::<_, i64>("PRAGMA auto_vacuum")
        .fetch_one(&pool)
        .await?;

    assert_eq!(
        (journal_mode, auto_vacuum),
        ("wal".to_string(), SqliteAutoVacuum::Incremental as i64)
    );
    pool.close().await;
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
async fn open_read_write_pool_preserves_wal_conversion_lock_error() -> anyhow::Result<()> {
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

    let result = sqlite.open_read_write_pool(&database_path).await;
    sqlx::query("ROLLBACK").execute(&mut *lock_holder).await?;
    drop(lock_holder);
    existing_pool.close().await;

    let error = result.expect_err("WAL conversion requires the writer lock");
    assert_eq!(
        error
            .downcast_ref::<sqlx::Error>()
            .and_then(sqlx::Error::as_database_error)
            .and_then(sqlx::error::DatabaseError::code),
        Some("5".into())
    );
    assert!(crate::sqlite_error_detail_is_lock(&error.to_string()));
    // Failure must release the connection so a later startup can succeed.
    sqlite
        .open_read_write_pool(&database_path)
        .await?
        .close()
        .await;
    Ok(())
}
