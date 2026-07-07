use crate::outgoing_message::ConnectionId;
use crate::outgoing_message::ConnectionRequestId;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadGoal;
use codex_app_server_protocol::ThreadHistoryBuilder;
use codex_app_server_protocol::ThreadSettings;
use codex_app_server_protocol::Turn;
use codex_app_server_protocol::TurnError;
use codex_core::CodexThread;
use codex_core::ThreadConfigSnapshot;
use codex_file_watcher::WatchRegistration;
use codex_protocol::ThreadId;
#[cfg(test)]
use codex_protocol::config_types::MultiAgentMode;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::RolloutItem;
use codex_rollout::state_db::StateDbHandle;
use codex_utils_path_uri::LegacyAppPathString;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::sync::Weak;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::sync::watch;
use tokio_util::task::TaskTracker;
use tracing::error;

type PendingInterruptQueue = Vec<ConnectionRequestId>;

pub(crate) struct PendingThreadResumeRequest {
    pub(crate) request_id: ConnectionRequestId,
    pub(crate) resume_handled_tx: oneshot::Sender<ConnectionReservationAction>,
    pub(crate) history_items: Vec<RolloutItem>,
    pub(crate) config_snapshot: ThreadConfigSnapshot,
    pub(crate) instruction_sources: Vec<LegacyAppPathString>,
    pub(crate) thread_summary: codex_app_server_protocol::Thread,
    pub(crate) emit_thread_goal_update: bool,
    pub(crate) thread_goal_state_db: Option<StateDbHandle>,
    pub(crate) include_turns: bool,
    pub(crate) initial_turns_page:
        Option<codex_app_server_protocol::ThreadResumeInitialTurnsPageParams>,
    pub(crate) redact_resume_payloads: bool,
}

// ThreadListenerCommand is used to perform operations in the context of the thread listener, for serialization purposes.
pub(crate) enum ThreadListenerCommand {
    // SendThreadResumeResponse is used to resume an already running thread by sending the thread's history to the client and atomically subscribing for new updates.
    SendThreadResumeResponse(Box<PendingThreadResumeRequest>),
    // EmitThreadGoalUpdated is used to order goal updates with running-thread resume responses and goal clears.
    EmitThreadGoalUpdated {
        turn_id: Option<String>,
        goal: ThreadGoal,
    },
    // EmitThreadGoalCleared is used to order app-server goal clears with running-thread resume responses.
    EmitThreadGoalCleared,
    // EmitThreadGoalSnapshot is used to read and emit the latest goal state in the listener order.
    EmitThreadGoalSnapshot {
        state_db: StateDbHandle,
    },
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
    pub(crate) started_at: Option<i64>,
    pub(crate) command_execution_started: HashSet<String>,
    pub(crate) last_error: Option<TurnError>,
}

#[derive(Default)]
pub(crate) struct ThreadState {
    pub(crate) pending_interrupts: PendingInterruptQueue,
    pub(crate) pending_rollbacks: Option<ConnectionRequestId>,
    pub(crate) turn_summary: TurnSummary,
    pub(crate) last_terminal_turn_id: Option<String>,
    pub(crate) cancel_tx: Option<oneshot::Sender<()>>,
    pub(crate) experimental_raw_events: bool,
    pub(crate) listener_generation: u64,
    last_thread_settings: Option<ThreadSettings>,
    listener_command_tx: Option<mpsc::UnboundedSender<ThreadListenerCommand>>,
    current_turn_history: ThreadHistoryBuilder,
    listener_thread: Option<Weak<CodexThread>>,
    watch_registration: WatchRegistration,
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
        watch_registration: WatchRegistration,
        thread_settings_baseline: ThreadSettings,
    ) -> (mpsc::UnboundedReceiver<ThreadListenerCommand>, u64) {
        if let Some(previous) = self.cancel_tx.replace(cancel_tx) {
            let _ = previous.send(());
        }
        self.listener_generation = self.listener_generation.wrapping_add(1);
        self.last_thread_settings = Some(thread_settings_baseline);
        let (listener_command_tx, listener_command_rx) = mpsc::unbounded_channel();
        self.listener_command_tx = Some(listener_command_tx);
        self.listener_thread = Some(Arc::downgrade(conversation));
        self.watch_registration = watch_registration;
        (listener_command_rx, self.listener_generation)
    }

    pub(crate) fn clear_listener(
        &mut self,
    ) -> Option<mpsc::UnboundedSender<ThreadListenerCommand>> {
        if let Some(cancel_tx) = self.cancel_tx.take() {
            let _ = cancel_tx.send(());
        }
        let listener_command_tx = self.listener_command_tx.take();
        self.current_turn_history.reset();
        self.listener_thread = None;
        self.watch_registration = WatchRegistration::default();
        listener_command_tx
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

    pub(crate) fn track_current_turn_event(&mut self, event_turn_id: &str, event: &EventMsg) {
        if let EventMsg::TurnStarted(payload) = event {
            self.turn_summary.started_at = payload.started_at;
        }
        self.current_turn_history.handle_event(event);
        if matches!(event, EventMsg::TurnAborted(_) | EventMsg::TurnComplete(_))
            && !self.current_turn_history.has_active_turn()
        {
            self.last_terminal_turn_id = Some(event_turn_id.to_string());
            self.current_turn_history.reset();
        }
    }

    pub(crate) fn note_thread_settings(&mut self, thread_settings: ThreadSettings) -> bool {
        let changed = self.last_thread_settings.as_ref() != Some(&thread_settings);
        self.last_thread_settings = Some(thread_settings);
        changed
    }
}

pub(crate) struct ThreadListenerHandle {
    command_tx: mpsc::UnboundedSender<ThreadListenerCommand>,
    abort_handle: Option<tokio::task::AbortHandle>,
}

impl ThreadListenerHandle {
    pub(crate) fn abort(&self) {
        if let Some(abort_handle) = &self.abort_handle {
            abort_handle.abort();
        }
    }

    pub(crate) async fn wait_until_closed(self) {
        if self.abort_handle.is_some() {
            self.command_tx.closed().await;
        }
    }
}

pub(crate) enum ThreadListenerRegistration {
    Registered(Option<ThreadListenerHandle>),
    Closing,
}

pub(crate) async fn resolve_server_request_on_thread_listener(
    thread_state: &Arc<Mutex<ThreadState>>,
    request_id: RequestId,
) {
    let (completion_tx, completion_rx) = oneshot::channel();
    let listener_command_tx = {
        let state = thread_state.lock().await;
        state.listener_command_tx()
    };
    let Some(listener_command_tx) = listener_command_tx else {
        error!("failed to remove pending client request: thread listener is not running");
        return;
    };

    if listener_command_tx
        .send(ThreadListenerCommand::ResolveServerRequest {
            request_id,
            completion_tx,
        })
        .is_err()
    {
        error!(
            "failed to remove pending client request: thread listener command channel is closed"
        );
        return;
    }

    if let Err(err) = completion_rx.await {
        error!("failed to remove pending client request: {err}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_app_server_protocol::ApprovalsReviewer;
    use codex_app_server_protocol::AskForApproval;
    use codex_app_server_protocol::SandboxPolicy;
    use codex_protocol::config_types::CollaborationMode;
    use codex_protocol::config_types::ModeKind;
    use codex_protocol::config_types::Settings;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use pretty_assertions::assert_eq;

    struct DropSignal(Option<oneshot::Sender<()>>);

    impl Drop for DropSignal {
        fn drop(&mut self) {
            if let Some(stopped) = self.0.take() {
                let _ = stopped.send(());
            }
        }
    }

    #[test]
    fn note_thread_settings_reports_only_effective_changes() {
        let mut state = ThreadState::default();
        let initial = thread_settings("mock-model");
        let updated = thread_settings("mock-model-2");

        let results = vec![
            state.note_thread_settings(initial.clone()),
            state.note_thread_settings(initial),
            state.note_thread_settings(updated.clone()),
            state.note_thread_settings(updated),
        ];

        assert_eq!(results, vec![true, false, true, false]);
    }

    #[test]
    fn stale_listener_cannot_unregister_replacement() {
        let manager = ThreadStateManager::new();
        let thread_id = ThreadId::new();
        let (old_tx, _old_rx) = mpsc::unbounded_channel();
        let (replacement_tx, _replacement_rx) = mpsc::unbounded_channel();
        manager.register_listener_command_tx(thread_id, old_tx.clone());
        manager.register_listener_command_tx(thread_id, replacement_tx.clone());

        assert!(
            manager
                .unregister_listener_command_tx(thread_id, &old_tx)
                .is_none()
        );
        assert!(
            manager
                .current_listener_command_tx(thread_id)
                .is_some_and(|current| current.same_channel(&replacement_tx))
        );
    }

    #[tokio::test]
    async fn clear_all_listeners_waits_for_aborted_tasks() {
        let manager = ThreadStateManager::new();
        let thread_id = ThreadId::new();
        let connection_id = ConnectionId(1);
        manager
            .connection_initialized(connection_id, ConnectionCapabilities::default())
            .await;
        let thread_state = manager.thread_state(thread_id).await;
        let (command_tx, command_rx) = mpsc::unbounded_channel();
        let (first_stopped_tx, first_stopped_rx) = oneshot::channel();
        let first_stopped = DropSignal(Some(first_stopped_tx));
        thread_state.lock().await.listener_command_tx = Some(command_tx.clone());
        let registration = manager.spawn_and_register_listener(thread_id, command_tx, async move {
            let _stopped = first_stopped;
            futures::future::pending::<()>().await;
            drop(command_rx);
        });
        assert!(matches!(
            registration,
            ThreadListenerRegistration::Registered(None)
        ));
        let (replacement_tx, replacement_rx) = mpsc::unbounded_channel();
        let (replacement_stopped_tx, replacement_stopped_rx) = oneshot::channel();
        let replacement_stopped = DropSignal(Some(replacement_stopped_tx));
        thread_state.lock().await.listener_command_tx = Some(replacement_tx.clone());
        let registration =
            manager.spawn_and_register_listener(thread_id, replacement_tx, async move {
                let _stopped = replacement_stopped;
                futures::future::pending::<()>().await;
                drop(replacement_rx);
            });
        assert!(matches!(
            registration,
            ThreadListenerRegistration::Registered(Some(_))
        ));

        tokio::time::timeout(
            std::time::Duration::from_secs(1),
            manager.clear_all_listeners(),
        )
        .await
        .expect("listener shutdown should not hang");
        first_stopped_rx
            .await
            .expect("superseded listener should stop");
        replacement_stopped_rx
            .await
            .expect("current listener should stop");
        assert!(
            manager
                .try_ensure_connection_subscribed(
                    ThreadId::new(),
                    connection_id,
                    /*experimental_raw_events*/ false,
                )
                .await
                .is_none()
        );
    }

    #[tokio::test]
    async fn detached_thread_state_is_rejected_for_listener_install() {
        let manager = ThreadStateManager::new();
        let thread_id = ThreadId::new();
        let detached = manager.thread_state(thread_id).await;
        let _ = manager.remove_thread_state(thread_id).await;
        let replacement = manager.thread_state(thread_id).await;

        assert!(!Arc::ptr_eq(&detached, &replacement));
        assert!(
            manager
                .lock_current_thread_state(thread_id, &detached)
                .await
                .is_none()
        );
    }

    fn thread_settings(model: &str) -> ThreadSettings {
        ThreadSettings {
            cwd: AbsolutePathBuf::from_absolute_path("/tmp").expect("absolute path"),
            approval_policy: AskForApproval::OnRequest,
            approvals_reviewer: ApprovalsReviewer::User,
            sandbox_policy: SandboxPolicy::ReadOnly {
                network_access: false,
            },
            active_permission_profile: None,
            model: model.to_string(),
            model_provider: "mock_provider".to_string(),
            service_tier: None,
            effort: None,
            summary: None,
            collaboration_mode: CollaborationMode {
                mode: ModeKind::Default,
                settings: Settings {
                    model: model.to_string(),
                    reasoning_effort: None,
                    developer_instructions: None,
                },
            },
            multi_agent_mode: MultiAgentMode::ExplicitRequestOnly,
            personality: None,
        }
    }
}

struct ThreadEntry {
    state: Arc<Mutex<ThreadState>>,
    connection_ids: HashSet<ConnectionId>,
    pending_connection_reservations: HashMap<ConnectionId, HashSet<u64>>,
    has_connections_watcher: watch::Sender<bool>,
}

impl Default for ThreadEntry {
    fn default() -> Self {
        Self {
            state: Arc::new(Mutex::new(ThreadState::default())),
            connection_ids: HashSet::new(),
            pending_connection_reservations: HashMap::new(),
            has_connections_watcher: watch::channel(false).0,
        }
    }
}

impl ThreadEntry {
    fn update_has_connections(&self) {
        let _ = self.has_connections_watcher.send_if_modified(|current| {
            let prev = *current;
            *current = !self.connection_ids.is_empty();
            prev != *current
        });
    }
}

#[derive(Default)]
struct ThreadStateManagerInner {
    live_connections: HashMap<ConnectionId, ConnectionCapabilities>,
    threads: HashMap<ThreadId, ThreadEntry>,
    thread_ids_by_connection: HashMap<ConnectionId, HashSet<ThreadId>>,
    next_connection_reservation_id: u64,
}

#[derive(Default)]
struct ThreadListenerRegistry {
    closing: bool,
    listeners: HashMap<ThreadId, ThreadListenerHandle>,
}

#[derive(Clone, Copy, Default)]
pub(crate) struct ConnectionCapabilities {
    pub(crate) request_attestation: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ConnectionReservationOutcome {
    Handled,
    Abandoned,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum ConnectionReservationAction {
    Commit,
    Cancel,
}

#[derive(Clone, Copy)]
struct ConnectionReservation {
    thread_id: ThreadId,
    connection_id: ConnectionId,
    reservation_id: u64,
}

#[derive(Clone, Default)]
pub(crate) struct ThreadStateManager {
    state: Arc<Mutex<ThreadStateManagerInner>>,
    // Extension event sinks are synchronous, so they need an await-free way to
    // enqueue work on the active per-thread listener.
    listener_commands: Arc<StdMutex<ThreadListenerRegistry>>,
    listener_tasks: TaskTracker,
}

impl ThreadStateManager {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) async fn connection_initialized(
        &self,
        connection_id: ConnectionId,
        capabilities: ConnectionCapabilities,
    ) {
        self.state
            .lock()
            .await
            .live_connections
            .insert(connection_id, capabilities);
    }

    pub(crate) async fn first_attestation_capable_connection_for_thread(
        &self,
        thread_id: ThreadId,
    ) -> Option<ConnectionId> {
        let state = self.state.lock().await;
        state
            .threads
            .get(&thread_id)?
            .connection_ids
            .iter()
            .filter_map(|connection_id| {
                state
                    .live_connections
                    .get(connection_id)?
                    .request_attestation
                    .then_some(*connection_id)
            })
            .min_by_key(|connection_id| connection_id.0)
    }

    pub(crate) async fn wait_for_thread_subscriber(&self, thread_id: ThreadId) {
        let mut has_connections = {
            let mut state = self.state.lock().await;
            state
                .threads
                .entry(thread_id)
                .or_default()
                .has_connections_watcher
                .subscribe()
        };
        while !*has_connections.borrow_and_update() {
            if has_connections.changed().await.is_err() {
                break;
            }
        }
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

    #[expect(
        clippy::await_holding_invalid_type,
        reason = "thread state identity must be checked while its lock is held"
    )]
    pub(crate) async fn lock_current_thread_state<'a>(
        &self,
        thread_id: ThreadId,
        expected: &'a Arc<Mutex<ThreadState>>,
    ) -> Option<tokio::sync::MutexGuard<'a, ThreadState>> {
        let guard = expected.lock().await;
        self.state
            .lock()
            .await
            .threads
            .get(&thread_id)
            .is_some_and(|entry| Arc::ptr_eq(&entry.state, expected))
            .then_some(guard)
    }

    pub(crate) fn current_listener_command_tx(
        &self,
        thread_id: ThreadId,
    ) -> Option<mpsc::UnboundedSender<ThreadListenerCommand>> {
        self.listener_commands
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .listeners
            .get(&thread_id)
            .map(|listener| listener.command_tx.clone())
    }

    #[cfg(test)]
    pub(crate) fn register_listener_command_tx(
        &self,
        thread_id: ThreadId,
        tx: mpsc::UnboundedSender<ThreadListenerCommand>,
    ) {
        let _ = self.register_listener(
            thread_id,
            ThreadListenerHandle {
                command_tx: tx,
                abort_handle: None,
            },
        );
    }

    pub(crate) fn spawn_and_register_listener<F>(
        &self,
        thread_id: ThreadId,
        command_tx: mpsc::UnboundedSender<ThreadListenerCommand>,
        listener: F,
    ) -> ThreadListenerRegistration
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        let mut registry = self
            .listener_commands
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if registry.closing {
            return ThreadListenerRegistration::Closing;
        }
        let listener = self.listener_tasks.spawn(listener);
        let handle = ThreadListenerHandle {
            command_tx,
            abort_handle: Some(listener.abort_handle()),
        };
        let retired = registry.listeners.insert(thread_id, handle);
        if let Some(retired) = &retired {
            retired.abort();
        }
        drop(listener);
        ThreadListenerRegistration::Registered(retired)
    }

    #[cfg(test)]
    fn register_listener(
        &self,
        thread_id: ThreadId,
        listener: ThreadListenerHandle,
    ) -> ThreadListenerRegistration {
        let mut registry = self
            .listener_commands
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if registry.closing {
            ThreadListenerRegistration::Closing
        } else {
            ThreadListenerRegistration::Registered(registry.listeners.insert(thread_id, listener))
        }
    }

    pub(crate) fn unregister_listener_command_tx(
        &self,
        thread_id: ThreadId,
        expected: &mpsc::UnboundedSender<ThreadListenerCommand>,
    ) -> Option<ThreadListenerHandle> {
        let mut listeners = self
            .listener_commands
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if listeners
            .listeners
            .get(&thread_id)
            .is_some_and(|current| current.command_tx.same_channel(expected))
        {
            listeners.listeners.remove(&thread_id)
        } else {
            None
        }
    }

    #[expect(
        clippy::await_holding_invalid_type,
        reason = "thread state removal must commit atomically after both locks are held"
    )]
    pub(crate) async fn remove_thread_state(
        &self,
        thread_id: ThreadId,
    ) -> Option<ThreadListenerHandle> {
        let thread_state = {
            let state = self.state.lock().await;
            state.threads.get(&thread_id)?.state.clone()
        };
        let mut thread_state_guard = self
            .lock_current_thread_state(thread_id, &thread_state)
            .await?;
        {
            let mut state = self.state.lock().await;
            state.threads.remove(&thread_id);
            state.thread_ids_by_connection.retain(|_, thread_ids| {
                thread_ids.remove(&thread_id);
                !thread_ids.is_empty()
            });
        }
        tracing::debug!(
            thread_id = %thread_id,
            listener_generation = thread_state_guard.listener_generation,
            had_listener = thread_state_guard.cancel_tx.is_some(),
            had_active_turn = thread_state_guard.active_turn_snapshot().is_some(),
            "clearing thread listener during thread-state teardown"
        );
        let listener_command_tx = thread_state_guard.clear_listener();
        drop(thread_state_guard);
        let listener =
            listener_command_tx.and_then(|tx| self.unregister_listener_command_tx(thread_id, &tx));
        if let Some(listener) = &listener {
            listener.abort();
        }
        listener
    }

    pub(crate) async fn clear_all_listeners(&self) {
        let listeners = {
            let mut registry = self
                .listener_commands
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            registry.closing = true;
            self.listener_tasks.close();
            registry
                .listeners
                .drain()
                .map(|(_, listener)| listener)
                .collect::<Vec<_>>()
        };
        for listener in &listeners {
            listener.abort();
        }
        let thread_states = {
            let state = self.state.lock().await;
            state
                .threads
                .iter()
                .map(|(thread_id, thread_entry)| (*thread_id, thread_entry.state.clone()))
                .collect::<Vec<_>>()
        };
        for (thread_id, thread_state) in thread_states {
            let mut thread_state = thread_state.lock().await;
            tracing::debug!(
                thread_id = %thread_id,
                listener_generation = thread_state.listener_generation,
                had_listener = thread_state.cancel_tx.is_some(),
                had_active_turn = thread_state.active_turn_snapshot().is_some(),
                "clearing thread listener during app-server shutdown"
            );
            let _ = thread_state.clear_listener();
        }
        drop(listeners);
        self.listener_tasks.wait().await;
    }

    pub(crate) async fn unsubscribe_connection_from_thread(
        &self,
        thread_id: ThreadId,
        connection_id: ConnectionId,
    ) -> bool {
        self.unsubscribe_connection_from_thread_if_state(
            thread_id,
            connection_id,
            /*expected*/ None,
        )
        .await
    }

    pub(crate) async fn unsubscribe_connection_from_current_thread_state(
        &self,
        thread_id: ThreadId,
        connection_id: ConnectionId,
        expected: &Arc<Mutex<ThreadState>>,
    ) -> bool {
        self.unsubscribe_connection_from_thread_if_state(
            thread_id,
            connection_id,
            /*expected*/ Some(expected),
        )
        .await
    }

    async fn unsubscribe_connection_from_thread_if_state(
        &self,
        thread_id: ThreadId,
        connection_id: ConnectionId,
        expected: Option<&Arc<Mutex<ThreadState>>>,
    ) -> bool {
        {
            let mut state = self.state.lock().await;
            if !state.threads.get(&thread_id).is_some_and(|entry| {
                expected.is_none_or(|expected| Arc::ptr_eq(&entry.state, expected))
            }) {
                return false;
            }

            if !state
                .thread_ids_by_connection
                .get(&connection_id)
                .is_some_and(|thread_ids| thread_ids.contains(&thread_id))
            {
                return false;
            }

            let removed = if let Some(thread_entry) = state.threads.get_mut(&thread_id) {
                let removed_subscription = thread_entry.connection_ids.remove(&connection_id);
                let canceled_reservation = thread_entry
                    .pending_connection_reservations
                    .remove(&connection_id)
                    .is_some();
                thread_entry.update_has_connections();
                removed_subscription || canceled_reservation
            } else {
                false
            };
            if !removed {
                return false;
            }
            if let Some(thread_ids) = state.thread_ids_by_connection.get_mut(&connection_id) {
                thread_ids.remove(&thread_id);
                if thread_ids.is_empty() {
                    state.thread_ids_by_connection.remove(&connection_id);
                }
            }
        };

        true
    }

    #[cfg(test)]
    pub(crate) async fn has_subscribers(&self, thread_id: ThreadId) -> bool {
        self.state
            .lock()
            .await
            .threads
            .get(&thread_id)
            .is_some_and(|thread_entry| !thread_entry.connection_ids.is_empty())
    }

    pub(crate) async fn has_connections_or_reservations(&self, thread_id: ThreadId) -> bool {
        self.state
            .lock()
            .await
            .threads
            .get(&thread_id)
            .is_some_and(|thread_entry| {
                !thread_entry.connection_ids.is_empty()
                    || !thread_entry.pending_connection_reservations.is_empty()
            })
    }

    pub(crate) async fn try_ensure_connection_subscribed(
        &self,
        thread_id: ThreadId,
        connection_id: ConnectionId,
        experimental_raw_events: bool,
    ) -> Option<Arc<Mutex<ThreadState>>> {
        let thread_state = {
            let mut state = self.state.lock().await;
            if self
                .listener_commands
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .closing
                || !state.live_connections.contains_key(&connection_id)
            {
                return None;
            }
            state
                .thread_ids_by_connection
                .entry(connection_id)
                .or_default()
                .insert(thread_id);
            let thread_entry = state.threads.entry(thread_id).or_default();
            thread_entry.connection_ids.insert(connection_id);
            thread_entry.update_has_connections();
            thread_entry.state.clone()
        };
        {
            let mut thread_state_guard = thread_state.lock().await;
            if experimental_raw_events {
                thread_state_guard.set_experimental_raw_events(/*enabled*/ true);
            }
        }
        Some(thread_state)
    }

    #[cfg(test)]
    pub(crate) async fn try_add_connection_to_thread(
        &self,
        thread_id: ThreadId,
        connection_id: ConnectionId,
    ) -> bool {
        let mut state = self.state.lock().await;
        if self
            .listener_commands
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .closing
            || !state.live_connections.contains_key(&connection_id)
        {
            return false;
        }
        state
            .thread_ids_by_connection
            .entry(connection_id)
            .or_default()
            .insert(thread_id);
        let thread_entry = state.threads.entry(thread_id).or_default();
        thread_entry.connection_ids.insert(connection_id);
        thread_entry.update_has_connections();
        true
    }

    pub(crate) async fn start_connection_reservation(
        &self,
        thread_id: ThreadId,
        connection_id: ConnectionId,
    ) -> Option<(
        oneshot::Sender<ConnectionReservationAction>,
        oneshot::Receiver<ConnectionReservationOutcome>,
    )> {
        let mut state = self.state.lock().await;
        let listener_registry = self
            .listener_commands
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if listener_registry.closing || !state.live_connections.contains_key(&connection_id) {
            return None;
        }
        let reservation_id = state.next_connection_reservation_id;
        let Some(next_reservation_id) = reservation_id.checked_add(1) else {
            error!("connection reservation id space exhausted");
            return None;
        };
        state.next_connection_reservation_id = next_reservation_id;
        state
            .thread_ids_by_connection
            .entry(connection_id)
            .or_default()
            .insert(thread_id);
        let thread_entry = state.threads.entry(thread_id).or_default();
        thread_entry
            .pending_connection_reservations
            .entry(connection_id)
            .or_default()
            .insert(reservation_id);
        let reservation = ConnectionReservation {
            thread_id,
            connection_id,
            reservation_id,
        };
        let (handled_tx, handled_rx) = oneshot::channel();
        let (outcome_tx, outcome_rx) = oneshot::channel();
        let manager = self.clone();
        let task = self.listener_tasks.spawn(async move {
            let (action, outcome) = match handled_rx.await {
                Ok(action) => (action, ConnectionReservationOutcome::Handled),
                Err(_) => (
                    ConnectionReservationAction::Cancel,
                    ConnectionReservationOutcome::Abandoned,
                ),
            };
            manager
                .finish_connection_reservation(reservation, action)
                .await;
            let _ = outcome_tx.send(outcome);
        });
        drop(task);
        drop(listener_registry);
        drop(state);
        Some((handled_tx, outcome_rx))
    }

    async fn finish_connection_reservation(
        &self,
        reservation: ConnectionReservation,
        action: ConnectionReservationAction,
    ) {
        let ConnectionReservation {
            thread_id,
            connection_id,
            reservation_id,
        } = reservation;
        let mut state = self.state.lock().await;
        let connection_is_live = state.live_connections.contains_key(&connection_id);
        let Some(thread_entry) = state.threads.get_mut(&thread_id) else {
            return;
        };
        let remove_reservation_set = {
            let Some(reservations) = thread_entry
                .pending_connection_reservations
                .get_mut(&connection_id)
            else {
                return;
            };
            if !reservations.remove(&reservation_id) {
                return;
            }
            reservations.is_empty()
        };
        if remove_reservation_set {
            thread_entry
                .pending_connection_reservations
                .remove(&connection_id);
        }
        let committed = matches!(action, ConnectionReservationAction::Commit) && connection_is_live;
        if committed {
            thread_entry.connection_ids.insert(connection_id);
        }
        let keep_connection_mapping = thread_entry.connection_ids.contains(&connection_id)
            || thread_entry
                .pending_connection_reservations
                .contains_key(&connection_id);
        thread_entry.update_has_connections();
        if !keep_connection_mapping
            && let Some(thread_ids) = state.thread_ids_by_connection.get_mut(&connection_id)
        {
            thread_ids.remove(&thread_id);
            if thread_ids.is_empty() {
                state.thread_ids_by_connection.remove(&connection_id);
            }
        }
    }

    pub(crate) async fn remove_connection(&self, connection_id: ConnectionId) -> Vec<ThreadId> {
        {
            let mut state = self.state.lock().await;
            state.live_connections.remove(&connection_id);
            let thread_ids = state
                .thread_ids_by_connection
                .remove(&connection_id)
                .unwrap_or_default();
            for thread_id in &thread_ids {
                if let Some(thread_entry) = state.threads.get_mut(thread_id) {
                    thread_entry.connection_ids.remove(&connection_id);
                    thread_entry
                        .pending_connection_reservations
                        .remove(&connection_id);
                    thread_entry.update_has_connections();
                }
            }
            thread_ids
                .into_iter()
                .filter(|thread_id| {
                    state.threads.get(thread_id).is_some_and(|thread_entry| {
                        thread_entry.connection_ids.is_empty()
                            && thread_entry.pending_connection_reservations.is_empty()
                    })
                })
                .collect::<Vec<_>>()
        }
    }

    pub(crate) async fn subscribe_to_has_connections(
        &self,
        thread_id: ThreadId,
    ) -> Option<watch::Receiver<bool>> {
        let state = self.state.lock().await;
        state
            .threads
            .get(&thread_id)
            .map(|thread_entry| thread_entry.has_connections_watcher.subscribe())
    }
}
