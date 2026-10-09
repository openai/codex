//! Owns the local database schema and pool registry shared by runtime handles and setup.
//! Initialization and deletion recovery share the registry lock to exclude stale pools.

use super::storage_error;
use codex_protocol::error::Result;
use codex_state::SqliteConfig;
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::LazyLock;
use std::sync::Weak;
use tokio::sync::Mutex;

pub(super) const DATABASE_FILE: &str = "agent_message_board_1.sqlite";

// Weak entries let the last board handle release its pool. Initialization and
// recovery share one lock so concurrent starts cannot open duplicate or stale pools.
pub(super) static POOLS: LazyLock<Mutex<HashMap<PathBuf, Weak<SqlitePool>>>> =
    LazyLock::new(Mutex::default);

pub(super) const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS board_templates (board TEXT PRIMARY KEY NOT NULL, template TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS deleted_boards (board TEXT PRIMARY KEY NOT NULL);
CREATE TABLE IF NOT EXISTS channels (
 board TEXT NOT NULL, name TEXT NOT NULL, name_search TEXT NOT NULL, created_at TEXT NOT NULL, timestamp INTEGER NOT NULL, author TEXT NOT NULL, description TEXT,
 PRIMARY KEY(board,name)
);
CREATE TABLE IF NOT EXISTS posts (
 seq INTEGER PRIMARY KEY AUTOINCREMENT,
 board TEXT NOT NULL, id TEXT NOT NULL, channel TEXT NOT NULL, root TEXT NOT NULL,
 author TEXT NOT NULL, timestamp INTEGER NOT NULL, body_search TEXT NOT NULL,
 payload TEXT NOT NULL, request_id TEXT NOT NULL, request TEXT NOT NULL,
 UNIQUE(board,id), UNIQUE(board,request_id)
);
CREATE INDEX IF NOT EXISTS posts_board_channel ON posts(board,channel,seq);
CREATE INDEX IF NOT EXISTS posts_board_channel_timestamp ON posts(board,channel,timestamp,seq);
CREATE INDEX IF NOT EXISTS posts_roots_created ON posts(board,channel,timestamp,seq) WHERE id=root;
CREATE INDEX IF NOT EXISTS posts_board_root ON posts(board,root,seq);
CREATE INDEX IF NOT EXISTS posts_board_root_timestamp ON posts(board,root,timestamp,seq);
CREATE INDEX IF NOT EXISTS posts_board_timestamp ON posts(board,timestamp,seq);
CREATE TABLE IF NOT EXISTS subscriptions (
 board TEXT NOT NULL, target TEXT NOT NULL, agent TEXT NOT NULL,
 PRIMARY KEY(board,target,agent)
);
CREATE TABLE IF NOT EXISTS subscription_opt_outs (
 board TEXT NOT NULL, target TEXT NOT NULL, agent TEXT NOT NULL,
 PRIMARY KEY(board,target,agent)
);";

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
