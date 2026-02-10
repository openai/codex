use anyhow::Result;
use chrono::DateTime;
use chrono::Utc;
use codex_protocol::ThreadId;
use sqlx::Row;
use sqlx::sqlite::SqliteRow;

/// Stored memory summaries for a single thread.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadMemory {
    pub thread_id: ThreadId,
    pub scope_kind: String,
    pub scope_key: String,
    pub raw_memory: String,
    pub memory_summary: String,
    pub updated_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub used_count: i64,
    pub invalidated_at: Option<DateTime<Utc>>,
    pub invalid_reason: Option<String>,
}

#[derive(Debug)]
pub(crate) struct ThreadMemoryRow {
    thread_id: String,
    scope_kind: String,
    scope_key: String,
    raw_memory: String,
    memory_summary: String,
    updated_at: i64,
    last_used_at: Option<i64>,
    used_count: i64,
    invalidated_at: Option<i64>,
    invalid_reason: Option<String>,
}

impl ThreadMemoryRow {
    pub(crate) fn try_from_row(row: &SqliteRow) -> Result<Self> {
        Ok(Self {
            thread_id: row.try_get("thread_id")?,
            scope_kind: row.try_get("scope_kind")?,
            scope_key: row.try_get("scope_key")?,
            raw_memory: row.try_get("raw_memory")?,
            memory_summary: row.try_get("memory_summary")?,
            updated_at: row.try_get("updated_at")?,
            last_used_at: row.try_get("last_used_at")?,
            used_count: row.try_get("used_count")?,
            invalidated_at: row.try_get("invalidated_at")?,
            invalid_reason: row.try_get("invalid_reason")?,
        })
    }
}

impl TryFrom<ThreadMemoryRow> for ThreadMemory {
    type Error = anyhow::Error;

    fn try_from(row: ThreadMemoryRow) -> std::result::Result<Self, Self::Error> {
        Ok(Self {
            thread_id: ThreadId::try_from(row.thread_id)?,
            scope_kind: row.scope_kind,
            scope_key: row.scope_key,
            raw_memory: row.raw_memory,
            memory_summary: row.memory_summary,
            updated_at: epoch_seconds_to_datetime(row.updated_at)?,
            last_used_at: row
                .last_used_at
                .map(epoch_seconds_to_datetime)
                .transpose()?,
            used_count: row.used_count,
            invalidated_at: row
                .invalidated_at
                .map(epoch_seconds_to_datetime)
                .transpose()?,
            invalid_reason: row.invalid_reason,
        })
    }
}

fn epoch_seconds_to_datetime(secs: i64) -> Result<DateTime<Utc>> {
    DateTime::<Utc>::from_timestamp(secs, 0)
        .ok_or_else(|| anyhow::anyhow!("invalid unix timestamp: {secs}"))
}
