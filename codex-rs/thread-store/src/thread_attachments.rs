//! Typed parameters for attachment operations on stored threads.

use crate::ThreadAttachmentArchiveFilter;
use codex_protocol::ThreadId;
use serde_json::Value;

/// Parameters for attaching a thread-owned attachment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AddThreadAttachmentParams {
    /// Thread that owns the attachment.
    pub thread_id: ThreadId,
    /// Client-defined attachment type.
    pub attachment_type: String,
    /// Stable attachment identity within its thread and type.
    pub identity_key: String,
    /// Bounded, client-defined attachment metadata.
    pub payload: Value,
}

/// Parameters for listing attachments owned by one thread.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListThreadAttachmentsParams {
    /// Thread whose attachments should be returned.
    pub thread_id: ThreadId,
    /// Opaque cursor returned by a previous attachment listing.
    pub cursor: Option<String>,
    /// Maximum number of attachments to return.
    pub limit: usize,
}

/// Parameters for removing a thread-owned attachment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoveThreadAttachmentParams {
    /// Thread that owns the attachment.
    pub thread_id: ThreadId,
    /// Client-defined attachment type.
    pub attachment_type: String,
    /// Stable attachment identity within its thread and type.
    pub identity_key: String,
}

/// Parameters for finding all owners of one attachment identity in this store.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListThreadAttachmentThreadsParams {
    pub attachment_type: String,
    pub identity_key: String,
    pub archive_filter: ThreadAttachmentArchiveFilter,
    pub cursor: Option<String>,
    pub limit: usize,
}
