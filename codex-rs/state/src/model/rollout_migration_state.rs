//! SQLite-backed bookkeeping types shared by rollout migrations.
//!
//! A migration cursor says “everything through this creation-ordered thread was checked.” A
//! skipped rollout stores enough file fingerprint information to tell whether a previously
//! unmigratable rollout is still unchanged.
//!
//! These types are intentionally generic so future rollout migrations can reuse the same state
//! shape without depending on legacy -> paginated policy.

use anyhow::Result;
use sqlx::Row;
use sqlx::sqlite::SqliteRow;

/// Creation-ordered thread frontier checked by one rollout migration.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct RolloutMigrationCursor {
    pub thread_created_at: i64,
    pub thread_id: String,
}

/// Persisted progress for one rollout migration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RolloutMigrationState {
    pub last_checked_thread: Option<RolloutMigrationCursor>,
}

impl RolloutMigrationState {
    pub(crate) fn try_from_row(row: &SqliteRow) -> Result<Self> {
        let thread_created_at = row.try_get("last_checked_thread_created_at")?;
        let thread_id = row.try_get("last_checked_thread_id")?;
        let last_checked_thread = match (thread_created_at, thread_id) {
            (Some(thread_created_at), Some(thread_id)) => Some(RolloutMigrationCursor {
                thread_created_at,
                thread_id,
            }),
            (None, None) => None,
            _ => {
                return Err(anyhow::anyhow!(
                    "rollout migration state has incomplete last checked thread"
                ));
            }
        };
        Ok(Self {
            last_checked_thread,
        })
    }
}

/// An unchanged rollout that one migration can safely skip.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RolloutMigrationSkippedRollout {
    pub rollout_path: String,
    pub rollout_size_bytes: i64,
    pub rollout_modified_at_ns: i64,
    pub skip_reason: String,
}

impl RolloutMigrationSkippedRollout {
    pub(crate) fn try_from_row(row: &SqliteRow) -> Result<Self> {
        Ok(Self {
            rollout_path: row.try_get("rollout_path")?,
            rollout_size_bytes: row.try_get("rollout_size_bytes")?,
            rollout_modified_at_ns: row.try_get("rollout_modified_at_ns")?,
            skip_reason: row.try_get("skip_reason")?,
        })
    }
}
