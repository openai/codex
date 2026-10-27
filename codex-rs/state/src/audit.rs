//! Read-only queries against an existing state database.

use anyhow::Result;
use codex_protocol::ThreadId;
use sqlx::Row;
use std::path::PathBuf;
use std::time::Duration;

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
    let pool = sqlite
        .open_read_only_pool(&sqlite.state_db_path(), /*busy_timeout*/ None)
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

/// Reads only the thread IDs and selected paths, without creating or migrating the state DB.
pub async fn read_thread_rollout_paths(sqlite: &SqliteConfig) -> Result<Vec<(ThreadId, PathBuf)>> {
    // Migration can scan the rollout headers immediately if the index is locked.
    let pool = sqlite
        .open_read_only_pool(&sqlite.state_db_path(), Some(Duration::ZERO))
        .await?;
    let rows = sqlx::query_as::<_, (String, String)>("SELECT id, rollout_path FROM threads")
        .fetch_all(&pool)
        .await?;
    pool.close().await;
    rows.into_iter()
        .map(|(id, path)| Ok((ThreadId::from_string(&id)?, PathBuf::from(path))))
        .collect()
}
