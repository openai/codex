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
    pub(crate) experimental_raw_events: bool,
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

    pub(crate) fn set_experimental_raw_events(&mut self, enabled: bool) {
        self.experimental_raw_events = enabled;
    }
}

#[derive(Clone, Copy)]
struct SubscriptionState {
    thread_id: ThreadId,
    connection_id: ConnectionId,
}

#[derive(Default)]
pub(crate) struct ThreadStateManager {
    thread_states: HashMap<ThreadId, Arc<Mutex<ThreadState>>>,
    subscription_state_by_id: HashMap<Uuid, SubscriptionState>,
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
        let subscription_state = self.subscription_state_by_id.remove(&subscription_id)?;
        let thread_id = subscription_state.thread_id;

        let connection_still_subscribed_to_thread =
            self.subscription_state_by_id.values().any(|state| {
                state.thread_id == thread_id
                    && state.connection_id == subscription_state.connection_id
            });
        if !connection_still_subscribed_to_thread {
            let mut remove_connection_entry = false;
            if let Some(thread_ids) = self
                .thread_ids_by_connection
                .get_mut(&subscription_state.connection_id)
            {
                thread_ids.remove(&thread_id);
                remove_connection_entry = thread_ids.is_empty();
            }
            if remove_connection_entry {
                self.thread_ids_by_connection
                    .remove(&subscription_state.connection_id);
            }
            if let Some(thread_state) = self.thread_states.get(&thread_id) {
                thread_state
                    .lock()
                    .await
                    .remove_connection(subscription_state.connection_id);
            }
        }

        if let Some(thread_state) = self.thread_states.get(&thread_id) {
            let mut thread_state = thread_state.lock().await;
            if thread_state.subscribed_connection_ids().is_empty() {
                thread_state.clear_listener();
            }
        }
        Some(thread_id)
    }

    pub(crate) async fn remove_thread_state(&mut self, thread_id: ThreadId) {
        if let Some(thread_state) = self.thread_states.remove(&thread_id) {
            thread_state.lock().await.clear_listener();
        }
        self.subscription_state_by_id
            .retain(|_, state| state.thread_id != thread_id);
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
        experimental_raw_events: bool,
    ) -> Arc<Mutex<ThreadState>> {
        self.subscription_state_by_id.insert(
            subscription_id,
            SubscriptionState {
                thread_id,
                connection_id,
            },
        );
        self.thread_ids_by_connection
            .entry(connection_id)
            .or_default()
            .insert(thread_id);
        let thread_state = self.thread_state(thread_id);
        {
            let mut thread_state_guard = thread_state.lock().await;
            thread_state_guard.add_connection(connection_id);
            thread_state_guard.set_experimental_raw_events(experimental_raw_events);
        }
        thread_state
    }

    pub(crate) async fn ensure_connection_subscribed(
        &mut self,
        thread_id: ThreadId,
        connection_id: ConnectionId,
        experimental_raw_events: bool,
    ) -> Arc<Mutex<ThreadState>> {
        self.thread_ids_by_connection
            .entry(connection_id)
            .or_default()
            .insert(thread_id);
        let thread_state = self.thread_state(thread_id);
        {
            let mut thread_state_guard = thread_state.lock().await;
            thread_state_guard.add_connection(connection_id);
            if experimental_raw_events {
                thread_state_guard.set_experimental_raw_events(true);
            }
        }
        thread_state
    }

    pub(crate) async fn remove_connection(&mut self, connection_id: ConnectionId) {
        let Some(thread_ids) = self.thread_ids_by_connection.remove(&connection_id) else {
            return;
        };
        self.subscription_state_by_id
            .retain(|_, state| state.connection_id != connection_id);

        for thread_id in thread_ids {
            if let Some(thread_state) = self.thread_states.get(&thread_id) {
                let mut thread_state = thread_state.lock().await;
                thread_state.remove_connection(connection_id);
                if thread_state.subscribed_connection_ids().is_empty() {
                    thread_state.clear_listener();
                }
            }
        }
    }
}
