use std::future::Future;
use std::pin::Pin;

use codex_protocol::protocol::ThreadGoalUpdatedEvent;

pub type ExtensionEventFuture<'a> = Pin<Box<dyn Future<Output = ()> + Send + 'a>>;

/// Events that extensions can ask the host to deliver.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExtensionEvent {
    ThreadGoalUpdated(ThreadGoalUpdatedEvent),
}

/// Host-provided sink for extension-generated events.
///
/// Extensions construct typed extension events, then leave persistence,
/// ordering, transport fanout, and logging decisions to the host.
pub trait ExtensionEventSink: Send + Sync {
    /// Queue one extension event for host-owned delivery.
    fn emit<'a>(&'a self, event: ExtensionEvent) -> ExtensionEventFuture<'a>;
}

/// Event sink used when the host does not expose extension event emission.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopExtensionEventSink;

impl ExtensionEventSink for NoopExtensionEventSink {
    fn emit<'a>(&'a self, _event: ExtensionEvent) -> ExtensionEventFuture<'a> {
        Box::pin(std::future::ready(()))
    }
}
