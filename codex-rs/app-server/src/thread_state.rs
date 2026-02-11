use crate::outgoing_message::ConnectionId;
use crate::outgoing_message::ConnectionRequestId;
use codex_app_server_protocol::TurnError;
use codex_core::CodexThread;
use codex_protocol::ThreadId;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::Weak;
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
    pub(crate) cancel_tx: Option<oneshot::Sender<()>>,
    listener_thread: Option<Weak<CodexThread>>,
    subscribed_connections: HashSet<ConnectionId>,
}

impl ThreadState {
    pub(crate) fn listener_matches(&self, conversation: &Arc<CodexThread>) -> bool {
        self.listener_thread
            .as_ref()
            .and_then(Weak::upgrade)
            .is_some_and(|existing| Arc::ptr_eq(&existing, conversation))
    }

    pub(crate) fn set_listener(
        &mut self,
        cancel_tx: oneshot::Sender<()>,
        conversation: &Arc<CodexThread>,
    ) {
        if let Some(previous) = self.cancel_tx.replace(cancel_tx) {
            let _ = previous.send(());
        }
        self.listener_thread = Some(Arc::downgrade(conversation));
    }

    pub(crate) fn clear_listener(&mut self) {
        if let Some(cancel_tx) = self.cancel_tx.take() {
            let _ = cancel_tx.send(());
        }
        self.listener_thread = None;
    }

    pub(crate) fn add_connection(&mut self, connection_id: ConnectionId) {
        self.subscribed_connections.insert(connection_id);
    }

    pub(crate) fn remove_connection(&mut self, connection_id: ConnectionId) {
        self.subscribed_connections.remove(&connection_id);
    }

    pub(crate) fn subscribed_connection_ids(&self) -> Vec<ConnectionId> {
        self.subscribed_connections.iter().copied().collect()
    }
}

#[derive(Default)]
pub(crate) struct ThreadStateManager {
    thread_states: HashMap<ThreadId, Arc<Mutex<ThreadState>>>,
    thread_id_by_subscription: HashMap<Uuid, ThreadId>,
    thread_ids_by_connection: HashMap<ConnectionId, HashSet<ThreadId>>,
}

impl ThreadStateManager {
    pub(crate) fn new() -> Self {
        Self::default()
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
            let mut state = thread_state.lock().await;
            // Back-compat: removing any subscription clears the active listener task for the
            // thread, but we intentionally do not delete other subscription IDs here.
            // TODO: Revisit once v1 listener lifecycle semantics are cleaned up.
            state.clear_listener();
        }
        Some(thread_id)
    }

    pub(crate) async fn remove_thread_state(&mut self, thread_id: ThreadId) {
        if let Some(thread_state) = self.thread_states.remove(&thread_id) {
            thread_state.lock().await.clear_listener();
        }
        self.thread_id_by_subscription
            .retain(|_, existing_thread_id| *existing_thread_id != thread_id);
        self.thread_ids_by_connection.retain(|_, thread_ids| {
            thread_ids.remove(&thread_id);
            !thread_ids.is_empty()
        });
    }

    pub(crate) async fn set_listener(
        &mut self,
        subscription_id: Uuid,
        thread_id: ThreadId,
        connection_id: ConnectionId,
    ) -> Arc<Mutex<ThreadState>> {
        self.thread_id_by_subscription
            .insert(subscription_id, thread_id);
        self.thread_ids_by_connection
            .entry(connection_id)
            .or_default()
            .insert(thread_id);
        let thread_state = self.thread_state(thread_id);
        thread_state.lock().await.add_connection(connection_id);
        thread_state
    }

    pub(crate) async fn ensure_connection_subscribed(
        &mut self,
        thread_id: ThreadId,
        connection_id: ConnectionId,
    ) -> Arc<Mutex<ThreadState>> {
        self.thread_ids_by_connection
            .entry(connection_id)
            .or_default()
            .insert(thread_id);
        let thread_state = self.thread_state(thread_id);
        thread_state.lock().await.add_connection(connection_id);
        thread_state
    }

    pub(crate) async fn remove_connection(&mut self, connection_id: ConnectionId) {
        let Some(thread_ids) = self.thread_ids_by_connection.remove(&connection_id) else {
            return;
        };
        for thread_id in thread_ids {
            if let Some(thread_state) = self.thread_states.get(&thread_id) {
                thread_state.lock().await.remove_connection(connection_id);
            }
        }
    }
}
