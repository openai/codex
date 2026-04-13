use sqlx::FromRow;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExternalMessageCreateParams {
    pub id: String,
    pub thread_id: String,
    pub source: String,
    pub content: String,
    pub instructions: Option<String>,
    pub meta_json: String,
    pub delivery: String,
    pub queued_at: i64,
}

impl ExternalMessageCreateParams {
    pub fn new(
        thread_id: String,
        source: String,
        content: String,
        instructions: Option<String>,
        meta_json: String,
        delivery: String,
        queued_at: i64,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            thread_id,
            source,
            content,
            instructions,
            meta_json,
            delivery,
            queued_at,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExternalMessage {
    pub seq: i64,
    pub id: String,
    pub thread_id: String,
    pub source: String,
    pub content: String,
    pub instructions: Option<String>,
    pub meta_json: String,
    pub delivery: String,
    pub queued_at: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExternalMessageClaim {
    Claimed(ExternalMessage),
    Invalid { id: String, reason: String },
    NotReady,
}

#[derive(Debug, FromRow)]
pub(crate) struct ExternalMessageRow {
    pub seq: i64,
    pub id: String,
    pub thread_id: String,
    pub source: String,
    pub content: String,
    pub instructions: Option<String>,
    pub meta_json: String,
    pub delivery: String,
    pub queued_at: i64,
}

impl From<ExternalMessageRow> for ExternalMessage {
    fn from(row: ExternalMessageRow) -> Self {
        Self {
            seq: row.seq,
            id: row.id,
            thread_id: row.thread_id,
            source: row.source,
            content: row.content,
            instructions: row.instructions,
            meta_json: row.meta_json,
            delivery: row.delivery,
            queued_at: row.queued_at,
        }
    }
}
