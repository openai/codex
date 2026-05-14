use chrono::DateTime;
use chrono::Utc;
use codex_protocol::ThreadId;
use serde_json::Value as JsonValue;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadArtifact {
    pub thread_id: ThreadId,
    pub id: String,
    pub created_at: DateTime<Utc>,
    pub artifact_type: String,
    pub payload: JsonValue,
}
