use anyhow::Result;
use codex_protocol::ThreadId;
use sqlx::Row;
use sqlx::sqlite::SqliteRow;

/// One durable, ordered user submission for a thread.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueuedUserSubmissionRecord {
    pub id: String,
    pub thread_id: ThreadId,
    pub payload: String,
}

impl QueuedUserSubmissionRecord {
    pub(crate) fn try_from_row(row: &SqliteRow) -> Result<Self> {
        Ok(Self {
            id: row.try_get("id")?,
            thread_id: ThreadId::try_from(row.try_get::<String, _>("thread_id")?)?,
            payload: row.try_get("payload_json")?,
        })
    }
}
