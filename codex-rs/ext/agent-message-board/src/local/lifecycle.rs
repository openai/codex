//! Permanent board deletion, serialized with writes across all open handles.
//!
//! Keep only a root ID tombstone so delayed writes cannot resurrect deleted data.

use super::DATABASE_FILE;
use super::LocalAgentMessageBoard;
use super::SCHEMA;
use super::invalid;
use super::storage_error;
use codex_protocol::SessionId;
use codex_protocol::error::Result;
use codex_state::SqliteConfig;
use sqlx::Sqlite;
use sqlx::Transaction;

impl LocalAgentMessageBoard {
    /// Permanently removes boards owned by these roots, including their posts and subscriptions.
    /// A child's ID does not match its parent's board. Unload and archive must not call this.
    /// Safe to retry and independent of whether the feature is currently enabled.
    pub async fn delete_boards(sqlite: &SqliteConfig, roots: &[SessionId]) -> Result<()> {
        let path = sqlite.home().join(DATABASE_FILE);
        if roots.is_empty() || !tokio::fs::try_exists(&path).await? {
            return Ok(());
        }
        let pool = sqlite
            .open_read_write_pool(&path)
            .await
            .map_err(storage_error)?;
        // Also handles databases created before permanent deletion was supported.
        sqlx::raw_sql(SCHEMA)
            .execute(&pool)
            .await
            .map_err(storage_error)?;
        let mut tx = pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(storage_error)?;
        for root in roots {
            sqlx::query("INSERT OR IGNORE INTO deleted_boards (board) VALUES (?)")
                .bind(root.to_string())
                .execute(&mut *tx)
                .await
                .map_err(storage_error)?;
            for statement in [
                "DELETE FROM subscriptions WHERE board=?",
                "DELETE FROM posts WHERE board=?",
                "DELETE FROM channels WHERE board=?",
            ] {
                sqlx::query(statement)
                    .bind(root.to_string())
                    .execute(&mut *tx)
                    .await
                    .map_err(storage_error)?;
            }
        }
        tx.commit().await.map_err(storage_error)?;
        pool.close().await;
        Ok(())
    }

    pub(super) async fn begin_write(&self) -> Result<Transaction<'_, Sqlite>> {
        let mut tx = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(storage_error)?;
        let deleted: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM deleted_boards WHERE board=?)")
                .bind(self.identity.to_string())
                .fetch_one(&mut *tx)
                .await
                .map_err(storage_error)?;
        if deleted {
            return Err(invalid(
                "the message board's root has been permanently deleted",
            ));
        }
        Ok(tx)
    }
}
