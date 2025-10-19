use std::sync::Arc;
use std::sync::Mutex;

use tokio::sync::mpsc::UnboundedSender;

use crate::app_event::AppEvent;
use crate::history_cell::HistoryCell;
use crate::session_log;

#[derive(Clone, Debug)]
pub(crate) struct AppEventSender {
    pub app_event_tx: UnboundedSender<AppEvent>,
    conversation_id: Arc<Mutex<Option<String>>>,
}

impl AppEventSender {
    pub(crate) fn new(app_event_tx: UnboundedSender<AppEvent>) -> Self {
        Self {
            app_event_tx,
            conversation_id: Arc::new(Mutex::new(None)),
        }
    }

    /// Create a scoped sender that shares the same channel but tracks a
    /// conversation-specific context independently.
    pub(crate) fn scoped(&self) -> Self {
        Self {
            app_event_tx: self.app_event_tx.clone(),
            conversation_id: Arc::new(Mutex::new(None)),
        }
    }

    pub(crate) fn set_conversation_id<S: Into<String>>(&self, conversation_id: S) {
        let mut guard = self
            .conversation_id
            .lock()
            .expect("conversation_id mutex poisoned");
        *guard = Some(conversation_id.into());
    }

    /// Send an event to the app event channel. If it fails, we swallow the
    /// error and log it.
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

    pub(crate) fn send_history_cell(&self, cell: Box<dyn HistoryCell>) {
        let conversation_id = self
            .conversation_id
            .lock()
            .expect("conversation_id mutex poisoned")
            .clone();
        if conversation_id.is_none() {
            tracing::error!("history cell emitted without conversation context");
        }
        self.send(AppEvent::InsertHistoryCell {
            conversation_id,
            cell,
        });
    }
}
