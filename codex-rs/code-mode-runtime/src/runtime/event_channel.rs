//! Ordered runtime event transport with backpressure for bounded collectors.

use tokio::sync::mpsc;

use super::RuntimeEvent;
use codex_code_mode_protocol::CodeModeOutputTruncation;

// Amortize small-write handoffs while keeping the backlog of raw payloads
// ahead of the bounded collector small.
const EVENT_CAPACITY: usize = 4;

#[derive(Clone)]
pub(crate) enum RuntimeEventSender {
    Legacy(mpsc::UnboundedSender<RuntimeEvent>),
    Bounded(mpsc::Sender<RuntimeEvent>),
}

pub(crate) enum RuntimeEventReceiver {
    Legacy(mpsc::UnboundedReceiver<RuntimeEvent>),
    Bounded(mpsc::Receiver<RuntimeEvent>),
}

pub(crate) fn runtime_event_channel(
    mode: CodeModeOutputTruncation,
) -> (RuntimeEventSender, RuntimeEventReceiver) {
    match mode {
        CodeModeOutputTruncation::Client => {
            let (tx, rx) = mpsc::unbounded_channel();
            (
                RuntimeEventSender::Legacy(tx),
                RuntimeEventReceiver::Legacy(rx),
            )
        }
        CodeModeOutputTruncation::Server => {
            let (tx, rx) = mpsc::channel(EVENT_CAPACITY);
            (
                RuntimeEventSender::Bounded(tx),
                RuntimeEventReceiver::Bounded(rx),
            )
        }
    }
}

impl From<mpsc::UnboundedSender<RuntimeEvent>> for RuntimeEventSender {
    fn from(value: mpsc::UnboundedSender<RuntimeEvent>) -> Self {
        Self::Legacy(value)
    }
}

impl RuntimeEventReceiver {
    pub(crate) async fn recv(&mut self) -> Option<RuntimeEvent> {
        match self {
            Self::Legacy(rx) => rx.recv().await,
            Self::Bounded(rx) => rx.recv().await,
        }
    }
}

impl RuntimeEventSender {
    /// Bounded sends run only on the dedicated V8 thread, preserving event order
    /// without making synchronous JavaScript output helpers return promises.
    /// Capacity limits the backlog between V8 and the cell actor.
    ///
    /// # Panics
    ///
    /// In server mode, panics if called from an async execution context.
    pub(crate) fn blocking_send(&self, event: RuntimeEvent) -> Result<(), ()> {
        match self {
            Self::Legacy(tx) => tx.send(event).map_err(|_| ()),
            Self::Bounded(tx) => tx.blocking_send(event).map_err(|_| ()),
        }
    }
}

#[cfg(test)]
#[path = "event_channel_tests.rs"]
mod tests;
