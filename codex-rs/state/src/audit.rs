//! Read-only state database queries for diagnostics.

use anyhow::Result;
use log::LevelFilter;
use sqlx::ConnectOptions;
use sqlx::Row;
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::sqlite::SqlitePoolOptions;
use std::path::Path;
use std::path::PathBuf;

/// Minimal thread metadata used by read-only state database audits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadStateAuditRow {
    pub id: String,
    pub rollout_path: PathBuf,
    pub archived: bool,
    pub source: String,
    pub model_provider: String,
}

/// Read persisted thread rows from a state DB without creating, migrating, or repairing it.
pub async fn read_thread_state_audit_rows(path: &Path) -> Result<Vec<ThreadStateAuditRow>> {
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(false)
        .read_only(true)
        .log_statements(LevelFilter::Off);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await?;
    let rows = sqlx::query(
        r#"
SELECT id, rollout_path, archived, source, model_provider
FROM threads
        "#,
    )
    .fetch_all(&pool)
    .await?;
    pool.close().await;

    rows.into_iter()
        .map(|row| {
            let archived: i64 = row.try_get("archived")?;
            Ok(ThreadStateAuditRow {
                id: row.try_get("id")?,
                rollout_path: PathBuf::from(row.try_get::<String, _>("rollout_path")?),
                archived: archived != 0,
                source: row.try_get("source")?,
                model_provider: row.try_get("model_provider")?,
            })
        })
        .collect()
}
