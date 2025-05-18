use std::sync::mpsc::Sender;

use crate::app_event::AppEvent;

#[derive(Clone, Debug)]
pub(crate) struct AppEventSender {
    app_event_tx: Sender<AppEvent>,
}

impl AppEventSender {
    pub(crate) fn new(app_event_tx: Sender<AppEvent>) -> Self {
        Self { app_event_tx }
    }

    /// Send an event to the app event channel. If it fails, we swallow the
    /// error and log it.
    pub(crate) fn send(&self, event: AppEvent) {
        if let Err(e) = self.app_event_tx.send(event) {
            tracing::error!("failed to send event: {e}");
        }
    }
}
