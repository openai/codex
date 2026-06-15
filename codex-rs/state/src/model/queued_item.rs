use anyhow::Result;
use anyhow::anyhow;
use chrono::DateTime;
use chrono::Utc;
use codex_protocol::ThreadId;
use sqlx::Row;
use sqlx::sqlite::SqliteRow;

use super::epoch_millis_to_datetime;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueuedItemState {
    Pending,
    Claimed,
    Failed,
}

impl TryFrom<&str> for QueuedItemState {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> Result<Self> {
        match value {
            "pending" => Ok(Self::Pending),
            "claimed" => Ok(Self::Claimed),
            "failed" => Ok(Self::Failed),
            other => Err(anyhow!("unknown queued item state `{other}`")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueuedItemRecord {
    pub queued_item_id: String,
    pub thread_id: ThreadId,
    pub payload_jsonb: Vec<u8>,
    pub queue_order: i64,
    pub state: QueuedItemState,
    pub failure_jsonb: Option<Vec<u8>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueuedItemClaim {
    pub item: QueuedItemRecord,
    pub claim_token: String,
}

pub(crate) struct QueuedItemRow {
    pub queued_item_id: String,
    pub thread_id: String,
    pub payload_jsonb: Vec<u8>,
    pub queue_order: i64,
    pub state: String,
    pub failure_jsonb: Option<Vec<u8>>,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

impl QueuedItemRow {
    pub(crate) fn try_from_row(row: &SqliteRow) -> Result<Self> {
        Ok(Self {
            queued_item_id: row.try_get("queued_item_id")?,
            thread_id: row.try_get("thread_id")?,
            payload_jsonb: row.try_get("payload_jsonb")?,
            queue_order: row.try_get("queue_order")?,
            state: row.try_get("state")?,
            failure_jsonb: row.try_get("failure_jsonb")?,
            created_at_ms: row.try_get("created_at_ms")?,
            updated_at_ms: row.try_get("updated_at_ms")?,
        })
    }
}

impl TryFrom<QueuedItemRow> for QueuedItemRecord {
    type Error = anyhow::Error;

    fn try_from(row: QueuedItemRow) -> Result<Self> {
        Ok(Self {
            queued_item_id: row.queued_item_id,
            thread_id: ThreadId::try_from(row.thread_id)?,
            payload_jsonb: row.payload_jsonb,
            queue_order: row.queue_order,
            state: QueuedItemState::try_from(row.state.as_str())?,
            failure_jsonb: row.failure_jsonb,
            created_at: epoch_millis_to_datetime(row.created_at_ms)?,
            updated_at: epoch_millis_to_datetime(row.updated_at_ms)?,
        })
    }
}
