use anyhow::Result;
use chrono::DateTime;
use chrono::Utc;
use codex_protocol::ThreadId;
use sqlx::Row;
use sqlx::sqlite::SqliteRow;

/// Stored stage-1 memory extraction output for a single thread.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stage1Output {
    pub thread_id: ThreadId,
    pub source_updated_at: DateTime<Utc>,
    pub raw_memory: String,
    pub summary: String,
    pub generated_at: DateTime<Utc>,
}

#[derive(Debug)]
pub(crate) struct Stage1OutputRow {
    thread_id: String,
    source_updated_at: i64,
    raw_memory: String,
    summary: String,
    generated_at: i64,
}

impl Stage1OutputRow {
    pub(crate) fn try_from_row(row: &SqliteRow) -> Result<Self> {
        Ok(Self {
            thread_id: row.try_get("thread_id")?,
            source_updated_at: row.try_get("source_updated_at")?,
            raw_memory: row.try_get("raw_memory")?,
            summary: row.try_get("summary")?,
            generated_at: row.try_get("generated_at")?,
        })
    }
}

impl TryFrom<Stage1OutputRow> for Stage1Output {
    type Error = anyhow::Error;

    fn try_from(row: Stage1OutputRow) -> std::result::Result<Self, Self::Error> {
        Ok(Self {
            thread_id: ThreadId::try_from(row.thread_id)?,
            source_updated_at: epoch_seconds_to_datetime(row.source_updated_at)?,
            raw_memory: row.raw_memory,
            summary: row.summary,
            generated_at: epoch_seconds_to_datetime(row.generated_at)?,
        })
    }
}

fn epoch_seconds_to_datetime(secs: i64) -> Result<DateTime<Utc>> {
    DateTime::<Utc>::from_timestamp(secs, 0)
        .ok_or_else(|| anyhow::anyhow!("invalid unix timestamp: {secs}"))
}
