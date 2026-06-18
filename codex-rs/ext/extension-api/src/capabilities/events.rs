use codex_protocol::ThreadId;
use codex_protocol::protocol::Event;

/// Host-provided fire-and-forget sink for extension-generated events.
///
/// Extensions construct protocol events with the correlation id appropriate for
/// the callback they are handling and identify the owning thread explicitly,
/// then leave persistence, ordering, transport fanout, and logging decisions to
/// the host.
pub trait ExtensionEventSink: Send + Sync {
    /// Queue one protocol event for host-owned delivery.
    fn emit(&self, thread_id: ThreadId, event: Event);
}

/// Event sink used when the host does not expose extension event emission.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopExtensionEventSink;

impl ExtensionEventSink for NoopExtensionEventSink {
    fn emit(&self, _thread_id: ThreadId, _event: Event) {}
}
