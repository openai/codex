//! Thread attachment records and outcomes; membership changes independently of resource contents.

use codex_protocol::ThreadId;
use serde_json::Value;

/// A bounded attachment durably associated with one thread.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadAttachment {
    /// Stable, server-assigned UUIDv7 attachment identity.
    pub id: String,
    /// Thread that owns this attachment.
    pub thread_id: ThreadId,
    /// Client-defined attachment category.
    pub attachment_type: String,
    /// Client-defined stable identity within the owning thread and attachment category.
    pub identity_key: String,
    /// Bounded, client-defined attachment metadata.
    pub payload: Value,
    /// Integer Unix timestamp in seconds when the attachment was attached.
    pub created_at: i64,
}

/// Result of attaching one uniquely identified thread attachment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AddThreadAttachmentOutcome {
    /// A new durable attachment was created.
    Created(ThreadAttachment),
    /// The attachment was already attached; its payload and creation time are unchanged.
    Existing(ThreadAttachment),
}

/// Result of removing a thread attachment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemoveThreadAttachmentOutcome {
    /// An attached attachment was removed.
    Removed(ThreadAttachment),
    /// No attachment with the requested identity was attached.
    NotFound,
}

/// One deterministically ordered page of attachments across selected threads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadAttachmentPage {
    /// Attachments ordered by thread identity, creation time, and attachment identity.
    pub attachments: Vec<ThreadAttachment>,
    /// Opaque cursor for the next page, or `None` when the selection is exhausted.
    pub next_cursor: Option<String>,
}

/// Selects attachment owners by their persisted archive state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreadAttachmentArchiveFilter {
    All,
    NonArchived,
    Archived,
}

/// A thread whose current attachment membership contains the requested resource.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadAttachmentOwner {
    pub thread_id: ThreadId,
    pub archived: bool,
}

/// One page of attachment owners, ordered by thread ID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadAttachmentOwnerPage {
    pub threads: Vec<ThreadAttachmentOwner>,
    pub next_cursor: Option<String>,
}
