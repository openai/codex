use codex_protocol::ThreadId;
use serde_json::Value;

/// A bounded artifact durably associated with one thread.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadArtifact {
    /// Stable, server-assigned UUIDv7 artifact identity.
    pub id: String,
    /// Thread that owns this artifact.
    pub thread_id: ThreadId,
    /// Client-defined artifact category.
    pub artifact_type: String,
    /// Client-defined stable identity within the owning thread and artifact category.
    pub identity_key: String,
    /// Bounded, client-defined artifact metadata.
    pub payload: Value,
    /// Integer Unix timestamp in seconds when the artifact was attached.
    pub created_at: i64,
}

/// Result of attaching one uniquely identified thread artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThreadArtifactAttachmentOutcome {
    /// A new durable artifact was created.
    Created(ThreadArtifact),
    /// The artifact was already attached; its payload and creation time are unchanged.
    Existing(ThreadArtifact),
}

/// Result of removing a thread artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThreadArtifactRemovalOutcome {
    /// An attached artifact was removed.
    Removed(ThreadArtifact),
    /// No artifact with the requested identity was attached.
    NotFound,
}

/// One deterministically ordered page of artifacts across selected threads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadArtifactPage {
    /// Artifacts ordered by thread identity, creation time, and artifact identity.
    pub artifacts: Vec<ThreadArtifact>,
    /// Opaque cursor for the next page, or `None` when the selection is exhausted.
    pub next_cursor: Option<String>,
}
