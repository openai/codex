use codex_protocol::protocol::Event;
use std::future::Future;
use std::pin::Pin;

pub type ExtensionEventFuture<'a> = Pin<Box<dyn Future<Output = ()> + Send + 'a>>;

/// Host-provided sink for extension-generated events.
///
/// Extensions construct protocol events with the correlation id appropriate for
/// the callback they are handling, then leave persistence, ordering, transport
/// fanout, and logging decisions to the host.
pub trait ExtensionEventSink: Send + Sync {
    /// Queue one protocol event for host-owned delivery.
    fn emit<'a>(&'a self, event: Event) -> ExtensionEventFuture<'a>;
}

/// Event sink used when the host does not expose extension event emission.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopExtensionEventSink;

impl ExtensionEventSink for NoopExtensionEventSink {
    fn emit<'a>(&'a self, _event: Event) -> ExtensionEventFuture<'a> {
        Box::pin(std::future::ready(()))
    }
}
