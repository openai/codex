use codex_protocol::models::ResponseItem;

/// Read-only conversation-history snapshot supplied by the extension host.
///
/// Implementations should retain the host's existing snapshot storage rather than
/// copying response payloads into an extension-owned collection.
pub trait ConversationHistorySnapshot: Send + Sync {
    /// Returns the snapshot's response items in conversation order.
    fn items(&self) -> Box<dyn Iterator<Item = &ResponseItem> + Send + '_>;
}
