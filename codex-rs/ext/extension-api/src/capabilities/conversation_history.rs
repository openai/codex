use codex_history::RetainedContext;

use codex_protocol::models::ResponseItem;

/// Read-only conversation-history snapshot supplied by the extension host.
///
/// Implementations should retain the host's existing snapshot storage rather than
/// copying response payloads into an extension-owned collection.
pub trait ConversationHistorySnapshot: Send + Sync {
    /// Returns the generation of the history captured by this snapshot.
    fn history_version(&self) -> u64;

    /// Host-owned revision captured with this snapshot. Advances on user messages and
    /// history resets, but stays unchanged for compaction and internal context.
    fn user_message_revision(&self) -> u64;

    /// Returns the snapshot's response items in conversation order.
    fn items(&self) -> Box<dyn Iterator<Item = &ResponseItem> + Send + '_>;

    /// Host-owned retained facts captured atomically with the parent model window.
    /// These facts may be available while review still uses a legacy transcript.
    fn retained_context(&self) -> Option<&RetainedContext> {
        None
    }

    /// Whether review uses the parent checkpoint and model window instead of a legacy transcript.
    /// Checkpoint compatibility is independent of access to retained user evidence.
    fn uses_parent_context_for_review(&self) -> bool {
        self.retained_context().is_some()
    }

    /// Producer compatibility recorded on the latest opaque checkpoint. Missing provenance
    /// must not be inferred from the currently selected model, including after resume.
    fn latest_compaction_model_hash(&self) -> Option<&str> {
        None
    }

    /// Original review evidence retained across parent compaction, in conversation order.
    /// Hosts without separate retention provide their current history.
    fn review_items(&self) -> Box<dyn Iterator<Item = &ResponseItem> + Send + '_> {
        self.items()
    }

    /// Changes whenever offsets into the retained review evidence become invalid.
    fn review_history_version(&self) -> u64 {
        self.history_version()
    }
}
