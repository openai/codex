use sqlx::FromRow;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThreadTimerCreateParams {
    pub id: String,
    pub thread_id: String,
    pub source: String,
    pub client_id: String,
    pub trigger_json: String,
    pub prompt: String,
    pub delivery: String,
    pub created_at: i64,
    pub next_run_at: Option<i64>,
    pub last_run_at: Option<i64>,
    pub pending_run: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThreadTimerUpdateParams {
    pub trigger_json: String,
    pub delivery: String,
    pub next_run_at: Option<i64>,
    pub last_run_at: Option<i64>,
    pub pending_run: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThreadTimer {
    pub id: String,
    pub thread_id: String,
    pub source: String,
    pub client_id: String,
    pub trigger_json: String,
    pub prompt: String,
    pub delivery: String,
    pub created_at: i64,
    pub next_run_at: Option<i64>,
    pub last_run_at: Option<i64>,
    pub pending_run: bool,
}

#[derive(Debug, FromRow)]
pub(crate) struct ThreadTimerRow {
    pub id: String,
    pub thread_id: String,
    pub source: String,
    pub client_id: String,
    pub trigger_json: String,
    pub prompt: String,
    pub delivery: String,
    pub created_at: i64,
    pub next_run_at: Option<i64>,
    pub last_run_at: Option<i64>,
    pub pending_run: i64,
}

impl From<ThreadTimerRow> for ThreadTimer {
    fn from(row: ThreadTimerRow) -> Self {
        Self {
            id: row.id,
            thread_id: row.thread_id,
            source: row.source,
            client_id: row.client_id,
            trigger_json: row.trigger_json,
            prompt: row.prompt,
            delivery: row.delivery,
            created_at: row.created_at,
            next_run_at: row.next_run_at,
            last_run_at: row.last_run_at,
            pending_run: row.pending_run != 0,
        }
    }
}
