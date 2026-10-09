//! Shared SQLite initialization used by runtime handles and local experiment setup.

use super::DATABASE_FILE;
use super::POOLS;
use super::SCHEMA;
use super::storage_error;
use codex_protocol::error::Result;
use codex_state::SqliteConfig;
use sqlx::SqlitePool;
use std::sync::Arc;
use std::sync::Weak;

#[expect(
    clippy::await_holding_invalid_type,
    reason = "serialize shared pool initialization"
)]
pub(super) async fn open(sqlite: &SqliteConfig) -> Result<Arc<SqlitePool>> {
    tokio::fs::create_dir_all(sqlite.home()).await?;
    let path = tokio::fs::canonicalize(sqlite.home())
        .await?
        .join(DATABASE_FILE);
    let mut pools = POOLS.lock().await;
    pools.retain(|_, pool| pool.strong_count() > 0);
    let pool = if let Some(pool) = pools
        .get(&path)
        .and_then(Weak::upgrade)
        .filter(|pool| !pool.is_closed())
    {
        pool
    } else {
        let pool = sqlite
            .open_read_write_pool(&path)
            .await
            .map_err(storage_error)?;
        let mut tx = pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(storage_error)?;
        sqlx::raw_sql(SCHEMA)
            .execute(&mut *tx)
            .await
            .map_err(storage_error)?;
        let has_description: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM pragma_table_info('channels') WHERE name='description')",
        )
        .fetch_one(&mut *tx)
        .await
        .map_err(storage_error)?;
        if !has_description {
            sqlx::query("ALTER TABLE channels ADD COLUMN description TEXT")
                .execute(&mut *tx)
                .await
                .map_err(storage_error)?;
        }
        tx.commit().await.map_err(storage_error)?;
        let pool = Arc::new(pool);
        pools.insert(path, Arc::downgrade(&pool));
        pool
    };
    Ok(pool)
}
