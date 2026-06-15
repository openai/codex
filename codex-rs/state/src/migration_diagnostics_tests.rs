use std::path::Path;

use pretty_assertions::assert_eq;
use sqlx::SqlitePool;
use sqlx::sqlite::SqliteConnectOptions;

use super::*;
use crate::StateRuntime;
use crate::migrations::STATE_MIGRATOR;
use crate::runtime::test_support::unique_temp_dir;

async fn state_inspection(sqlite_home: &Path) -> RuntimeDbMigrationInspection {
    inspect_runtime_db_migrations(sqlite_home)
        .await
        .into_iter()
        .find(|inspection| inspection.label == "state DB")
        .expect("state inspection")
}

async fn state_pool(sqlite_home: &Path) -> SqlitePool {
    SqlitePool::connect_with(
        SqliteConnectOptions::new()
            .filename(state_db_path(sqlite_home))
            .create_if_missing(false),
    )
    .await
    .expect("state pool")
}

#[tokio::test]
async fn missing_runtime_databases_are_reported_without_creation() {
    let sqlite_home = unique_temp_dir();

    let inspections = inspect_runtime_db_migrations(&sqlite_home).await;

    assert_eq!(inspections.len(), 4);
    assert!(
        inspections
            .iter()
            .all(|inspection| inspection.status == RuntimeDbMigrationStatus::Missing)
    );
    assert!(!sqlite_home.exists());
}

#[tokio::test]
async fn current_runtime_database_is_compatible() {
    let sqlite_home = unique_temp_dir();
    let runtime = StateRuntime::init(sqlite_home.clone(), "test-provider".to_string())
        .await
        .expect("runtime");
    runtime.close().await;

    let inspection = state_inspection(&sqlite_home).await;

    assert_eq!(
        inspection.status,
        RuntimeDbMigrationStatus::Compatible {
            applied: STATE_MIGRATOR.iter().count(),
            pending: 0,
        }
    );
}

#[tokio::test]
async fn changed_checksum_is_incompatible() {
    let sqlite_home = unique_temp_dir();
    let runtime = StateRuntime::init(sqlite_home.clone(), "test-provider".to_string())
        .await
        .expect("runtime");
    runtime.close().await;
    let pool = state_pool(&sqlite_home).await;
    sqlx::query("UPDATE _sqlx_migrations SET checksum = ? WHERE version = 36")
        .bind(vec![0_u8; 48])
        .execute(&pool)
        .await
        .expect("change checksum");
    pool.close().await;

    let inspection = state_inspection(&sqlite_home).await;

    assert_eq!(
        inspection.status,
        RuntimeDbMigrationStatus::Incompatible(vec![RuntimeDbMigrationIssue::ChecksumMismatch {
            version: 36
        }])
    );
}

#[tokio::test]
async fn dirty_migration_is_incompatible() {
    let sqlite_home = unique_temp_dir();
    let runtime = StateRuntime::init(sqlite_home.clone(), "test-provider".to_string())
        .await
        .expect("runtime");
    runtime.close().await;
    let pool = state_pool(&sqlite_home).await;
    sqlx::query("UPDATE _sqlx_migrations SET success = FALSE WHERE version = 36")
        .execute(&pool)
        .await
        .expect("mark dirty");
    pool.close().await;

    let inspection = state_inspection(&sqlite_home).await;

    assert_eq!(
        inspection.status,
        RuntimeDbMigrationStatus::Incompatible(vec![RuntimeDbMigrationIssue::Dirty {
            version: 36,
        }])
    );
}

#[tokio::test]
async fn future_migration_is_tolerated() {
    let sqlite_home = unique_temp_dir();
    let runtime = StateRuntime::init(sqlite_home.clone(), "test-provider".to_string())
        .await
        .expect("runtime");
    runtime.close().await;
    let pool = state_pool(&sqlite_home).await;
    sqlx::query(
        "INSERT INTO _sqlx_migrations (version, description, success, checksum, execution_time) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(/*value*/ 9_999_i64)
    .bind("future migration")
    .bind(/*value*/ true)
    .bind(vec![1_u8, 2, 3, 4])
    .bind(/*value*/ 1_i64)
    .execute(&pool)
    .await
    .expect("insert future migration");
    pool.close().await;

    let inspection = state_inspection(&sqlite_home).await;

    assert_eq!(
        inspection.status,
        RuntimeDbMigrationStatus::Compatible {
            applied: STATE_MIGRATOR.iter().count() + 1,
            pending: 0,
        }
    );
}
