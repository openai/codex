use std::collections::HashMap;

use codex_protocol::protocol::UserSubmission;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum QueuedItemProvenance {
    User,
    ExternalEvent {
        source: String,
        metadata: HashMap<String, Value>,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "version")]
pub(crate) enum StoredQueuedItemPayload {
    #[serde(rename = "1")]
    V1 {
        submission: UserSubmission,
        provenance: QueuedItemProvenance,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct QueuedItem {
    pub id: String,
    pub submission: UserSubmission,
    pub provenance: QueuedItemProvenance,
    pub status: QueuedItemStatus,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QueuedItemStatus {
    Pending,
    Failed { error: String },
}

impl StoredQueuedItemPayload {
    pub(crate) fn into_parts(self) -> (UserSubmission, QueuedItemProvenance) {
        match self {
            Self::V1 {
                submission,
                provenance,
            } => (submission, provenance),
        }
    }
}
