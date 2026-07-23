//! Read-only state database queries for diagnostics.

use anyhow::Result;
use sqlx::Row;
use std::path::PathBuf;

use crate::SqliteConfig;

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
pub async fn read_thread_state_audit_rows(
    sqlite: &SqliteConfig,
) -> Result<Vec<ThreadStateAuditRow>> {
    let pool = sqlite.open_read_only_pool(&sqlite.state_db_path()).await?;
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
