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
    pub raw_memory: String,
    pub memory_summary: String,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug)]
pub(crate) struct ThreadMemoryRow {
    thread_id: String,
    raw_memory: String,
    memory_summary: String,
    updated_at: i64,
}

impl ThreadMemoryRow {
    pub(crate) fn try_from_row(row: &SqliteRow) -> Result<Self> {
        Ok(Self {
            thread_id: row.try_get("thread_id")?,
            raw_memory: row.try_get("raw_memory")?,
            memory_summary: row.try_get("memory_summary")?,
            updated_at: row.try_get("updated_at")?,
        })
    }
}

impl TryFrom<ThreadMemoryRow> for ThreadMemory {
    type Error = anyhow::Error;

    fn try_from(row: ThreadMemoryRow) -> std::result::Result<Self, Self::Error> {
        Ok(Self {
            thread_id: ThreadId::try_from(row.thread_id)?,
            raw_memory: row.raw_memory,
            memory_summary: row.memory_summary,
            updated_at: epoch_seconds_to_datetime(row.updated_at)?,
        })
    }
}

fn epoch_seconds_to_datetime(secs: i64) -> Result<DateTime<Utc>> {
    DateTime::<Utc>::from_timestamp(secs, 0)
        .ok_or_else(|| anyhow::anyhow!("invalid unix timestamp: {secs}"))
}
