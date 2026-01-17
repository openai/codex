//! App event channel wrapper with logging semantics.
//!
//! `AppEventSender` centralizes how UI components send [`AppEvent`] messages to
//! the main loop, ensuring session logging stays consistent and send failures
//! are surfaced without panicking.

use tokio::sync::mpsc::UnboundedSender;

use crate::app_event::AppEvent;
use crate::session_log;

/// Lightweight handle for sending [`AppEvent`] values to the app loop.
#[derive(Clone, Debug)]
pub(crate) struct AppEventSender {
    /// Channel used to deliver events to the main loop.
    pub app_event_tx: UnboundedSender<AppEvent>,
}

impl AppEventSender {
    /// Wrap the provided channel for consistent app event delivery.
    pub(crate) fn new(app_event_tx: UnboundedSender<AppEvent>) -> Self {
        Self { app_event_tx }
    }

    /// Send an event to the app event channel, logging failures instead of panicking.
    pub(crate) fn send(&self, event: AppEvent) {
        // Record inbound events for high-fidelity session replay.
        // Avoid double-logging Ops; those are logged at the point of submission.
        if !matches!(event, AppEvent::CodexOp(_)) {
            session_log::log_inbound_app_event(&event);
        }
        if let Err(e) = self.app_event_tx.send(event) {
            tracing::error!("failed to send event: {e}");
        }
    }
}
