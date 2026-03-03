use crate::outgoing_message::ConnectionId;
use crate::outgoing_message::ConnectionRequestId;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadHistoryBuilder;
use codex_app_server_protocol::Turn;
use codex_app_server_protocol::TurnError;
use codex_core::CodexThread;
use codex_core::ThreadConfigSnapshot;
use codex_protocol::ThreadId;
use codex_protocol::protocol::EventMsg;
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Weak;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use uuid::Uuid;

type PendingInterruptQueue = Vec<(
    ConnectionRequestId,
    crate::codex_message_processor::ApiVersion,
)>;

pub(crate) struct PendingThreadResumeRequest {
    pub(crate) request_id: ConnectionRequestId,
    pub(crate) rollout_path: PathBuf,
    pub(crate) config_snapshot: ThreadConfigSnapshot,
}

// ThreadListenerCommand is used to perform operations in the context of the thread listener, for serialization purposes.
pub(crate) enum ThreadListenerCommand {
    // SendThreadResumeResponse is used to resume an already running thread by sending the thread's history to the client and atomically subscribing for new updates.
    SendThreadResumeResponse(Box<PendingThreadResumeRequest>),
    // ResolveServerRequest is used to notify the client that the request has been resolved.
    // It is executed in the thread listener's context to ensure that the resolved notification is ordered with regard to the request itself.
    ResolveServerRequest {
        request_id: RequestId,
        completion_tx: oneshot::Sender<()>,
    },
}

/// Per-conversation accumulation of the latest states e.g. error message while a turn runs.
#[derive(Default, Clone)]
pub(crate) struct TurnSummary {
    pub(crate) file_change_started: HashSet<String>,
    pub(crate) command_execution_started: HashSet<String>,
    pub(crate) last_error: Option<TurnError>,
}

#[derive(Default)]
pub(crate) struct ThreadState {
    pub(crate) pending_interrupts: PendingInterruptQueue,
    pub(crate) pending_rollbacks: Option<ConnectionRequestId>,
    pub(crate) turn_summary: TurnSummary,
    pub(crate) cancel_tx: Option<oneshot::Sender<()>>,
    pub(crate) experimental_raw_events: bool,
    pub(crate) listener_generation: u64,
    listener_command_tx: Option<mpsc::UnboundedSender<ThreadListenerCommand>>,
    current_turn_history: ThreadHistoryBuilder,
    listener_thread: Option<Weak<CodexThread>>,
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
    ) -> (mpsc::UnboundedReceiver<ThreadListenerCommand>, u64) {
        if let Some(previous) = self.cancel_tx.replace(cancel_tx) {
            let _ = previous.send(());
        }
        self.listener_generation = self.listener_generation.wrapping_add(1);
        let (listener_command_tx, listener_command_rx) = mpsc::unbounded_channel();
        self.listener_command_tx = Some(listener_command_tx);
        self.listener_thread = Some(Arc::downgrade(conversation));
        (listener_command_rx, self.listener_generation)
    }

    pub(crate) fn clear_listener(&mut self) {
        if let Some(cancel_tx) = self.cancel_tx.take() {
            let _ = cancel_tx.send(());
        }
        self.listener_command_tx = None;
        self.current_turn_history.reset();
        self.listener_thread = None;
    }

    pub(crate) fn set_experimental_raw_events(&mut self, enabled: bool) {
        self.experimental_raw_events = enabled;
    }

    pub(crate) fn listener_command_tx(
        &self,
    ) -> Option<mpsc::UnboundedSender<ThreadListenerCommand>> {
        self.listener_command_tx.clone()
    }

    pub(crate) fn active_turn_snapshot(&self) -> Option<Turn> {
        self.current_turn_history.active_turn_snapshot()
    }

    pub(crate) fn track_current_turn_event(&mut self, event: &EventMsg) {
        self.current_turn_history.handle_event(event);
        if !self.current_turn_history.has_active_turn() {
            self.current_turn_history.reset();
        }
    }
}

#[derive(Clone, Copy)]
struct SubscriptionState {
    thread_id: ThreadId,
    connection_id: ConnectionId,
}

struct ThreadEntry {
    state: Arc<Mutex<ThreadState>>,
    connection_ids: HashSet<ConnectionId>,
}

impl Default for ThreadEntry {
    fn default() -> Self {
        Self {
            state: Arc::new(Mutex::new(ThreadState::default())),
            connection_ids: HashSet::new(),
        }
    }
}

#[derive(Default)]
struct ThreadStateManagerInner {
    live_connections: HashSet<ConnectionId>,
    threads: HashMap<ThreadId, ThreadEntry>,
    subscription_state_by_id: HashMap<Uuid, SubscriptionState>,
    thread_ids_by_connection: HashMap<ConnectionId, HashSet<ThreadId>>,
}

#[derive(Clone, Default)]
pub(crate) struct ThreadStateManager {
    state: Arc<Mutex<ThreadStateManagerInner>>,
}

impl ThreadStateManager {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) async fn connection_initialized(&self, connection_id: ConnectionId) {
        self.state
            .lock()
            .await
            .live_connections
            .insert(connection_id);
    }

    pub(crate) async fn subscribed_connection_ids(&self, thread_id: ThreadId) -> Vec<ConnectionId> {
        let state = self.state.lock().await;
        state
            .threads
            .get(&thread_id)
            .map(|thread_entry| thread_entry.connection_ids.iter().copied().collect())
            .unwrap_or_default()
    }

    pub(crate) async fn thread_state(&self, thread_id: ThreadId) -> Arc<Mutex<ThreadState>> {
        let mut state = self.state.lock().await;
        state.threads.entry(thread_id).or_default().state.clone()
    }

    pub(crate) async fn remove_listener(&self, subscription_id: Uuid) -> Option<ThreadId> {
        let (subscription_state, connection_still_subscribed_to_thread, thread_state) = {
            let mut state = self.state.lock().await;
            let subscription_state = state.subscription_state_by_id.remove(&subscription_id)?;
            let thread_id = subscription_state.thread_id;

            let connection_still_subscribed_to_thread = state
                .subscription_state_by_id
                .values()
                .any(|subscription_state_entry| {
                    subscription_state_entry.thread_id == thread_id
                        && subscription_state_entry.connection_id
                            == subscription_state.connection_id
                });
            if !connection_still_subscribed_to_thread {
                let mut remove_connection_entry = false;
                if let Some(thread_ids) = state
                    .thread_ids_by_connection
                    .get_mut(&subscription_state.connection_id)
                {
                    thread_ids.remove(&thread_id);
                    remove_connection_entry = thread_ids.is_empty();
                }
                if remove_connection_entry {
                    state
                        .thread_ids_by_connection
                        .remove(&subscription_state.connection_id);
                }
                if let Some(thread_entry) = state.threads.get_mut(&thread_id) {
                    thread_entry
                        .connection_ids
                        .remove(&subscription_state.connection_id);
                }
            }

            let thread_state = state.threads.get(&thread_id).map(|thread_entry| {
                (
                    thread_entry.connection_ids.is_empty(),
                    thread_entry.state.clone(),
                )
            });
            (
                subscription_state,
                connection_still_subscribed_to_thread,
                thread_state,
            )
        };
        let thread_id = subscription_state.thread_id;

        if let Some((no_subscribers, thread_state)) = thread_state {
            let thread_state = thread_state.lock().await;
            if !connection_still_subscribed_to_thread && no_subscribers {
                tracing::debug!(
                    thread_id = %thread_id,
                    subscription_id = %subscription_id,
                    connection_id = ?subscription_state.connection_id,
                    listener_generation = thread_state.listener_generation,
                    "retaining thread listener after last subscription removed"
                );
            }
        }
        Some(thread_id)
    }

    pub(crate) async fn remove_thread_state(&self, thread_id: ThreadId) {
        let thread_state = {
            let mut state = self.state.lock().await;
            let thread_state = state
                .threads
                .remove(&thread_id)
                .map(|thread_entry| thread_entry.state);
            state
                .subscription_state_by_id
                .retain(|_, state| state.thread_id != thread_id);
            state.thread_ids_by_connection.retain(|_, thread_ids| {
                thread_ids.remove(&thread_id);
                !thread_ids.is_empty()
            });
            thread_state
        };

        if let Some(thread_state) = thread_state {
            let mut thread_state = thread_state.lock().await;
            tracing::debug!(
                thread_id = %thread_id,
                listener_generation = thread_state.listener_generation,
                had_listener = thread_state.cancel_tx.is_some(),
                had_active_turn = thread_state.active_turn_snapshot().is_some(),
                "clearing thread listener during thread-state teardown"
            );
            thread_state.clear_listener();
        }
    }

    pub(crate) async fn unsubscribe_connection_from_thread(
        &self,
        thread_id: ThreadId,
        connection_id: ConnectionId,
    ) -> bool {
        {
            let mut state = self.state.lock().await;
            if !state.threads.contains_key(&thread_id) {
                return false;
            }

            if !state
                .thread_ids_by_connection
                .get(&connection_id)
                .is_some_and(|thread_ids| thread_ids.contains(&thread_id))
            {
                return false;
            }

            if let Some(thread_ids) = state.thread_ids_by_connection.get_mut(&connection_id) {
                thread_ids.remove(&thread_id);
                if thread_ids.is_empty() {
                    state.thread_ids_by_connection.remove(&connection_id);
                }
            }
            if let Some(thread_entry) = state.threads.get_mut(&thread_id) {
                thread_entry.connection_ids.remove(&connection_id);
            }

            state
                .subscription_state_by_id
                .retain(|_, subscription_state| {
                    !(subscription_state.thread_id == thread_id
                        && subscription_state.connection_id == connection_id)
                });
        };

        true
    }

    pub(crate) async fn has_subscribers(&self, thread_id: ThreadId) -> bool {
        self.state
            .lock()
            .await
            .threads
            .get(&thread_id)
            .is_some_and(|thread_entry| !thread_entry.connection_ids.is_empty())
    }

    pub(crate) async fn set_listener(
        &self,
        subscription_id: Uuid,
        thread_id: ThreadId,
        connection_id: ConnectionId,
        experimental_raw_events: bool,
    ) -> Arc<Mutex<ThreadState>> {
        let thread_state = {
            let mut state = self.state.lock().await;
            state.subscription_state_by_id.insert(
                subscription_id,
                SubscriptionState {
                    thread_id,
                    connection_id,
                },
            );
            state
                .thread_ids_by_connection
                .entry(connection_id)
                .or_default()
                .insert(thread_id);
            let thread_entry = state.threads.entry(thread_id).or_default();
            thread_entry.connection_ids.insert(connection_id);
            thread_entry.state.clone()
        };
        {
            let mut thread_state_guard = thread_state.lock().await;
            thread_state_guard.set_experimental_raw_events(experimental_raw_events);
        }
        thread_state
    }

    pub(crate) async fn try_ensure_connection_subscribed(
        &self,
        thread_id: ThreadId,
        connection_id: ConnectionId,
        experimental_raw_events: bool,
    ) -> Option<Arc<Mutex<ThreadState>>> {
        let thread_state = {
            let mut state = self.state.lock().await;
            if !state.live_connections.contains(&connection_id) {
                return None;
            }
            state
                .thread_ids_by_connection
                .entry(connection_id)
                .or_default()
                .insert(thread_id);
            let thread_entry = state.threads.entry(thread_id).or_default();
            thread_entry.connection_ids.insert(connection_id);
            thread_entry.state.clone()
        };
        {
            let mut thread_state_guard = thread_state.lock().await;
            if experimental_raw_events {
                thread_state_guard.set_experimental_raw_events(true);
            }
        }
        Some(thread_state)
    }

    pub(crate) async fn try_add_connection_to_thread(
        &self,
        thread_id: ThreadId,
        connection_id: ConnectionId,
    ) -> bool {
        let mut state = self.state.lock().await;
        if !state.live_connections.contains(&connection_id) {
            return false;
        }
        state
            .thread_ids_by_connection
            .entry(connection_id)
            .or_default()
            .insert(thread_id);
        state
            .threads
            .entry(thread_id)
            .or_default()
            .connection_ids
            .insert(connection_id);
        true
    }

    pub(crate) async fn remove_connection(&self, connection_id: ConnectionId) {
        let thread_states = {
            let mut state = self.state.lock().await;
            state.live_connections.remove(&connection_id);
            let thread_ids = state
                .thread_ids_by_connection
                .remove(&connection_id)
                .unwrap_or_default();
            state
                .subscription_state_by_id
                .retain(|_, state| state.connection_id != connection_id);
            for thread_id in &thread_ids {
                if let Some(thread_entry) = state.threads.get_mut(thread_id) {
                    thread_entry.connection_ids.remove(&connection_id);
                }
            }
            thread_ids
                .into_iter()
                .map(|thread_id| {
                    (
                        thread_id,
                        state
                            .threads
                            .get(&thread_id)
                            .is_none_or(|thread_entry| thread_entry.connection_ids.is_empty()),
                        state
                            .threads
                            .get(&thread_id)
                            .map(|thread_entry| thread_entry.state.clone()),
                    )
                })
                .collect::<Vec<_>>()
        };

        for (thread_id, no_subscribers, thread_state) in thread_states {
            if !no_subscribers {
                continue;
            }
            let Some(thread_state) = thread_state else {
                continue;
            };
            let listener_generation = thread_state.lock().await.listener_generation;
            tracing::debug!(
                thread_id = %thread_id,
                connection_id = ?connection_id,
                listener_generation,
                "retaining thread listener after connection disconnect left zero subscribers"
            );
        }
    }
}
