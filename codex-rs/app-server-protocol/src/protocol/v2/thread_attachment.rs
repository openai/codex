//! Wire contracts for managing the current set of attachments on a thread.

use crate::JsonSchema;
use crate::TS;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value as JsonValue;

/// An independently persisted attachment associated with a thread.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadAttachment {
    pub id: String,
    pub attachment_type: String,
    pub identity_key: String,
    pub payload: JsonValue,
    #[ts(type = "number")]
    pub created_at: i64,
}

/// Parameters for creating or locating an attachment on its owning thread.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadAttachmentAddParams {
    pub thread_id: String,
    pub attachment_type: String,
    pub identity_key: String,
    pub payload: JsonValue,
}

/// Result of attempting to associate an attachment with a thread.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase", export_to = "v2/")]
pub enum ThreadAttachmentAddOutcome {
    Created,
    Existing,
}

/// The created or existing attachment.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadAttachmentAddResponse {
    pub outcome: ThreadAttachmentAddOutcome,
    pub attachment: ThreadAttachment,
}

/// Parameters for listing attachments from one thread.
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadAttachmentListParams {
    pub thread_id: String,
    #[ts(optional = nullable)]
    pub cursor: Option<String>,
    #[ts(optional = nullable)]
    pub limit: Option<u32>,
}

/// One page of attachments associated with the requested thread.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadAttachmentListResponse {
    pub data: Vec<ThreadAttachment>,
    pub next_cursor: Option<String>,
}

/// Parameters for deleting an attachment by its stable thread-local identity.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadAttachmentRemoveParams {
    pub thread_id: String,
    pub attachment_type: String,
    pub identity_key: String,
}

/// Successful deletion does not return additional attachment data.
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadAttachmentRemoveResponse {}

/// The persisted attachment change represented by a notification.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase", export_to = "v2/")]
pub enum ThreadAttachmentOperation {
    Created,
    Deleted,
}

/// Notification published after a thread attachment is created or deleted.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadAttachmentUpdatedNotification {
    pub thread_id: String,
    pub attachment_type: String,
    pub identity_key: String,
    pub attachment_id: String,
    pub operation: ThreadAttachmentOperation,
}
