use crate::outgoing_message::ConnectionRequestId;
use codex_app_server_protocol::TurnError;
use codex_protocol::ThreadId;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::oneshot;
use uuid::Uuid;

type PendingInterruptQueue = Vec<(
    ConnectionRequestId,
    crate::codex_message_processor::ApiVersion,
)>;

/// Per-conversation accumulation of the latest states e.g. error message while a turn runs.
#[derive(Default, Clone)]
pub(crate) struct TurnSummary {
    pub(crate) file_change_started: HashSet<String>,
    pub(crate) last_error: Option<TurnError>,
}

#[derive(Default)]
pub(crate) struct ThreadState {
    pub(crate) pending_interrupts: PendingInterruptQueue,
    pub(crate) pending_rollbacks: Option<ConnectionRequestId>,
    pub(crate) turn_summary: TurnSummary,
    pub(crate) listener_cancel_txs: HashMap<Uuid, oneshot::Sender<()>>,
}

impl ThreadState {
    fn set_listener(&mut self, subscription_id: Uuid, cancel_tx: oneshot::Sender<()>) {
        if let Some(previous) = self.listener_cancel_txs.insert(subscription_id, cancel_tx) {
            let _ = previous.send(());
        }
    }

    fn clear_listener(&mut self, subscription_id: Uuid) {
        if let Some(cancel_tx) = self.listener_cancel_txs.remove(&subscription_id) {
            let _ = cancel_tx.send(());
        }
    }

    fn clear_listeners(&mut self) {
        for (_, cancel_tx) in self.listener_cancel_txs.drain() {
            let _ = cancel_tx.send(());
        }
    }
}

#[derive(Default)]
pub(crate) struct ThreadStateManager {
    thread_states: HashMap<ThreadId, Arc<Mutex<ThreadState>>>,
    thread_id_by_subscription: HashMap<Uuid, ThreadId>,
}

impl ThreadStateManager {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn has_listener_for_thread(&self, thread_id: ThreadId) -> bool {
        self.thread_id_by_subscription
            .values()
            .any(|existing| *existing == thread_id)
    }

    pub(crate) fn thread_state(&mut self, thread_id: ThreadId) -> Arc<Mutex<ThreadState>> {
        self.thread_states
            .entry(thread_id)
            .or_insert_with(|| Arc::new(Mutex::new(ThreadState::default())))
            .clone()
    }

    pub(crate) async fn remove_listener(&mut self, subscription_id: Uuid) -> Option<ThreadId> {
        let thread_id = self.thread_id_by_subscription.remove(&subscription_id)?;
        if let Some(thread_state) = self.thread_states.get(&thread_id) {
            thread_state.lock().await.clear_listener(subscription_id);
        }
        Some(thread_id)
    }

    pub(crate) async fn remove_thread_state(&mut self, thread_id: ThreadId) {
        if let Some(thread_state) = self.thread_states.remove(&thread_id) {
            thread_state.lock().await.clear_listeners();
        }
        self.thread_id_by_subscription
            .retain(|_, existing_thread_id| *existing_thread_id != thread_id);
    }

    pub(crate) async fn set_listener(
        &mut self,
        subscription_id: Uuid,
        thread_id: ThreadId,
        cancel_tx: oneshot::Sender<()>,
    ) -> Arc<Mutex<ThreadState>> {
        self.thread_id_by_subscription
            .insert(subscription_id, thread_id);
        let thread_state = self.thread_state(thread_id);
        thread_state
            .lock()
            .await
            .set_listener(subscription_id, cancel_tx);
        thread_state
    }
}
