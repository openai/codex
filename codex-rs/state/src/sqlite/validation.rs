//! Budgeted integrity checks that report confirmed corruption to database owners.
//!
//! Each file identity is checked at most once per shared SQLite configuration.
//! Attempts are recorded before validation, including failed or cancelled checks.

use file_id::FileId;
use sqlx::SqlitePool;
use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;
use tokio::sync::Mutex;

#[derive(Debug, Eq, PartialEq)]
pub(super) enum QuickCheckOutcome {
    Complete,
    Incomplete,
    CorruptedNeedsFixed,
    Skipped,
}

/// Keeps track of what databases were verified with quick_check() already
#[derive(Clone, Debug, Default)]
pub(super) struct SqliteQuickCheckManager(Arc<Mutex<HashSet<FileId>>>);

impl SqliteQuickCheckManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns this attempt's outcome, or `Skipped` if validation was already attempted.
    pub async fn quick_check_once(
        &self,
        pool: &SqlitePool,
        path: &Path,
        budget: Duration,
    ) -> anyhow::Result<QuickCheckOutcome> {
        let id = file_id::get_file_id(path)?;

        if !self.0.lock().await.insert(id) {
            // Only check each file once for this owner.
            return Ok(QuickCheckOutcome::Skipped);
        }

        quick_check(pool, Instant::now() + budget).await
    }
}

pub(super) async fn quick_check(
    pool: &SqlitePool,
    deadline: Instant,
) -> anyhow::Result<QuickCheckOutcome> {
    let mut connection = pool.acquire().await?;
    connection
        .lock_handle()
        .await?
        .set_progress_handler(/*num_ops*/ 1_000, move || Instant::now() < deadline);

    // quick_check(1) returns either "ok" or the first corruption finding
    let result = sqlx::query_scalar::<_, String>("PRAGMA quick_check(1)")
        .fetch_one(&mut *connection)
        .await;
    connection.lock_handle().await?.remove_progress_handler();

    connection.return_to_pool().await;
    match result.map_err(anyhow::Error::from) {
        Ok(result) if result == "ok" => Ok(QuickCheckOutcome::Complete),
        Ok(_) => Ok(QuickCheckOutcome::CorruptedNeedsFixed),
        Err(error) if crate::is_sqlite_corruption_error(&error) => {
            Ok(QuickCheckOutcome::CorruptedNeedsFixed)
        }
        // timeouts or other unhandled errors
        Err(_) => Ok(QuickCheckOutcome::Incomplete),
    }
}

#[cfg(test)]
#[path = "validation_tests.rs"]
mod tests;
