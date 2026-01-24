//! Multi-process index lock.
//!
//! Prevents concurrent indexing of the same workspace.

use std::time::Duration;
use std::time::Instant;

use crate::error::Result;
use crate::error::RetrievalErr;
use crate::storage::SqliteStore;

use std::sync::Arc;

/// Index lock guard (RAII).
///
/// Automatically releases the lock when dropped.
pub struct IndexLockGuard {
    db: Arc<SqliteStore>,
    holder_id: String,
    workspace: String,
}

impl IndexLockGuard {
    /// Explicitly release the lock.
    ///
    /// This is the preferred way to release the lock as it returns an error
    /// if the release fails. The `Drop` implementation provides best-effort
    /// cleanup but cannot report errors.
    ///
    /// After calling this method, the guard is consumed and `Drop` will not run.
    pub async fn unlock(self) -> Result<()> {
        let ws = self.workspace.clone();
        let hid = self.holder_id.clone();

        let result = self
            .db
            .query(move |conn| {
                conn.execute(
                    "DELETE FROM index_lock WHERE workspace = ? AND holder_id = ?",
                    rusqlite::params![ws, hid],
                )?;
                Ok(())
            })
            .await;

        // Prevent Drop from running again by forgetting self
        // SAFETY: We've already released the lock, Drop would be a no-op
        std::mem::forget(self);

        result
    }

    /// Try to acquire a lock with timeout.
    ///
    /// Uses atomic DELETE + INSERT to avoid check-then-act race conditions.
    /// Implements exponential backoff for retry to reduce contention.
    ///
    /// Returns an error if the lock cannot be acquired within the timeout.
    pub async fn try_acquire(
        db: Arc<SqliteStore>,
        workspace: &str,
        timeout: Duration,
    ) -> Result<Self> {
        let deadline = Instant::now() + timeout;
        let holder_id = format!(
            "{}_{}",
            std::process::id(),
            chrono::Utc::now().timestamp_millis()
        );

        let ws_check = workspace.to_string();
        let mut retry_count: u32 = 0;

        loop {
            let now = chrono::Utc::now().timestamp();
            let expires_at = now + 30; // 30 seconds timeout
            let ws = ws_check.clone();
            let hid = holder_id.clone();

            // Atomic operation: clean up expired locks AND try to acquire in single query
            // This eliminates the check-then-act race condition
            let acquired = db
                .query(move |conn| {
                    // First, clean up any expired locks for this workspace
                    let _ = conn.execute(
                        "DELETE FROM index_lock WHERE workspace = ? AND expires_at < ?",
                        rusqlite::params![&ws, now],
                    );

                    // Then try to insert our lock (will fail if another valid lock exists)
                    let count = conn.execute(
                        "INSERT OR IGNORE INTO index_lock (id, holder_id, workspace, locked_at, expires_at)
                         VALUES (1, ?, ?, ?, ?)",
                        rusqlite::params![hid, ws, now, expires_at],
                    )?;
                    Ok(count > 0)
                })
                .await?;

            if acquired {
                return Ok(Self {
                    db,
                    holder_id,
                    workspace: ws_check,
                });
            }

            // Check timeout
            if Instant::now() > deadline {
                return Err(RetrievalErr::SqliteLockedTimeout {
                    path: db.path().to_path_buf(),
                    waited_ms: timeout.as_millis() as u64,
                });
            }

            // Exponential backoff: 50ms, 100ms, 200ms, 400ms, 800ms (max 1s)
            retry_count += 1;
            let backoff_ms = 50 * (1u64 << retry_count.min(4));
            tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
        }
    }

    /// Refresh the lock (extend expiration time).
    pub async fn refresh(&self) -> Result<()> {
        let now = chrono::Utc::now().timestamp();
        let expires_at = now + 30;
        let ws = self.workspace.clone();
        let hid = self.holder_id.clone();

        self.db
            .query(move |conn| {
                let _ = conn.execute(
                    "UPDATE index_lock SET expires_at = ? WHERE workspace = ? AND holder_id = ?",
                    rusqlite::params![expires_at, ws, hid],
                );
                Ok(())
            })
            .await?;

        Ok(())
    }
}

impl Drop for IndexLockGuard {
    fn drop(&mut self) {
        // Best-effort asynchronous release using spawn.
        //
        // IMPORTANT: We use spawn() instead of block_on() to avoid deadlocks.
        // block_on() can deadlock if:
        // - The runtime is shutting down
        // - We're already inside an async context
        // - The DB operation requires the same thread that's blocked
        //
        // The tradeoff is that the lock might not be released immediately,
        // but SQLite's expires_at field provides a fallback timeout mechanism.
        let db = self.db.clone();
        let workspace = self.workspace.clone();
        let holder_id = self.holder_id.clone();

        // Try to get the current tokio runtime handle
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            // Spawn a task to release the lock asynchronously
            // This won't block Drop and avoids potential deadlocks
            handle.spawn(async move {
                let result = db
                    .query(move |conn| {
                        conn.execute(
                            "DELETE FROM index_lock WHERE workspace = ? AND holder_id = ?",
                            rusqlite::params![workspace, holder_id],
                        )?;
                        Ok(())
                    })
                    .await;

                if let Err(e) = result {
                    tracing::warn!(
                        error = %e,
                        "Failed to release index lock in Drop (async cleanup)"
                    );
                }
            });
        } else {
            // No runtime available - this can happen during shutdown.
            // The lock will be cleaned up by the expires_at timeout mechanism.
            tracing::debug!(
                workspace = %self.workspace,
                "Cannot release index lock: no tokio runtime available, relying on expiration"
            );
        }
    }
}
