use anyhow::Result;
use anyhow::anyhow;
use chrono::DateTime;
use chrono::Utc;
use codex_protocol::ThreadId;
use sqlx::Row;
use sqlx::sqlite::SqliteRow;

use super::epoch_millis_to_datetime;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreadQueuedTurnState {
    Pending,
    Dispatching,
    Failed,
}

impl ThreadQueuedTurnState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Dispatching => "dispatching",
            Self::Failed => "failed",
        }
    }
}

impl TryFrom<&str> for ThreadQueuedTurnState {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> Result<Self> {
        match value {
            "pending" => Ok(Self::Pending),
            "dispatching" => Ok(Self::Dispatching),
            "failed" => Ok(Self::Failed),
            other => Err(anyhow!("unknown thread queued turn state `{other}`")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadQueuedTurn {
    pub queued_turn_id: String,
    pub thread_id: ThreadId,
    pub turn_start_params_jsonb: Vec<u8>,
    pub queue_order: i64,
    pub state: ThreadQueuedTurnState,
    pub dispatch_turn_id: Option<String>,
    pub failure_jsonb: Option<Vec<u8>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub(crate) struct ThreadQueuedTurnRow {
    pub queued_turn_id: String,
    pub thread_id: String,
    pub turn_start_params_jsonb: Vec<u8>,
    pub queue_order: i64,
    pub state: String,
    pub dispatch_turn_id: Option<String>,
    pub failure_jsonb: Option<Vec<u8>>,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

impl ThreadQueuedTurnRow {
    pub(crate) fn try_from_row(row: &SqliteRow) -> Result<Self> {
        Ok(Self {
            queued_turn_id: row.try_get("queued_turn_id")?,
            thread_id: row.try_get("thread_id")?,
            turn_start_params_jsonb: row.try_get("turn_start_params_jsonb")?,
            queue_order: row.try_get("queue_order")?,
            state: row.try_get("state")?,
            dispatch_turn_id: row.try_get("dispatch_turn_id")?,
            failure_jsonb: row.try_get("failure_jsonb")?,
            created_at_ms: row.try_get("created_at_ms")?,
            updated_at_ms: row.try_get("updated_at_ms")?,
        })
    }
}

impl TryFrom<ThreadQueuedTurnRow> for ThreadQueuedTurn {
    type Error = anyhow::Error;

    fn try_from(row: ThreadQueuedTurnRow) -> Result<Self> {
        Ok(Self {
            queued_turn_id: row.queued_turn_id,
            thread_id: ThreadId::try_from(row.thread_id)?,
            turn_start_params_jsonb: row.turn_start_params_jsonb,
            queue_order: row.queue_order,
            state: ThreadQueuedTurnState::try_from(row.state.as_str())?,
            dispatch_turn_id: row.dispatch_turn_id,
            failure_jsonb: row.failure_jsonb,
            created_at: epoch_millis_to_datetime(row.created_at_ms)?,
            updated_at: epoch_millis_to_datetime(row.updated_at_ms)?,
        })
    }
}
