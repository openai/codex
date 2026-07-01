use crate::outgoing_message::ConnectionId;
use crate::outgoing_message::ConnectionRequestId;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadGoal;
use codex_app_server_protocol::ThreadHistoryBuilder;
use codex_app_server_protocol::ThreadSettings;
use codex_app_server_protocol::Turn;
use codex_app_server_protocol::TurnError;
use codex_core::CodexThread;
use codex_core::ThreadConfigSnapshot;
use codex_core::ThreadHistoryReconciliationOutcome;
use codex_file_watcher::WatchRegistration;
use codex_protocol::ThreadId;
#[cfg(test)]
use codex_protocol::config_types::MultiAgentMode;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::RolloutItem;
use codex_rollout::state_db::StateDbHandle;
use codex_thread_store::StoredThread;
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
use tracing::error;

type PendingInterruptQueue = Vec<ConnectionRequestId>;

pub(crate) struct PendingThreadRollback {
    pub(crate) request_id: ConnectionRequestId,
    completion_tx: Option<oneshot::Sender<()>>,
}

impl PendingThreadRollback {
    pub(crate) fn new(request_id: ConnectionRequestId, completion_tx: oneshot::Sender<()>) -> Self {
        Self {
            request_id,
            completion_tx: Some(completion_tx),
        }
    }

    pub(crate) fn complete(mut self) {
        if let Some(completion_tx) = self.completion_tx.take() {
            let _ = completion_tx.send(());
        }
    }
}

pub(crate) struct PendingThreadResumeRequest {
    pub(crate) request_id: ConnectionRequestId,
    pub(crate) app_server_client_name: Option<String>,
    pub(crate) app_server_client_version: Option<String>,
    pub(crate) config_snapshot: ThreadConfigSnapshot,
    pub(crate) instruction_sources: Vec<LegacyAppPathString>,
    pub(crate) emit_thread_goal_update: bool,
    pub(crate) thread_goal_state_db: Option<StateDbHandle>,
    pub(crate) include_turns: bool,
    pub(crate) initial_turns_page:
        Option<codex_app_server_protocol::ThreadResumeInitialTurnsPageParams>,
    pub(crate) redact_resume_payloads: bool,
    pub(crate) _resume_lease: ThreadPointOperationLease,
}

pub(crate) struct PreparedThreadResumeHistory {
    pub(crate) stored_thread: StoredThread,
    pub(crate) history_items: Vec<RolloutItem>,
    pub(crate) reconciliation_outcome: ThreadHistoryReconciliationOutcome,
}

// ThreadListenerCommand is used to perform operations in the context of the thread listener, for serialization purposes.
pub(crate) enum ThreadListenerCommand {
    // SendThreadResumeResponse is used to resume an already running thread by sending the thread's history to the client and atomically subscribing for new updates.
    SendThreadResumeResponse {
        request: Box<PendingThreadResumeRequest>,
        completion_tx: oneshot::Sender<()>,
    },
    // Completes the storage phase of a running-thread resume. The worker retains the core event
    // cut until `release_event_cut_tx` is dropped, so the listener can drain the exact pre-cut
    // event queue before subscribing the requesting connection.
    FinishThreadResumeResponse {
        request: Box<PendingThreadResumeRequest>,
        completion_tx: oneshot::Sender<()>,
        history_result: Box<Result<PreparedThreadResumeHistory, JSONRPCErrorError>>,
        release_event_cut_tx: oneshot::Sender<()>,
    },
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
    pub(crate) pending_rollbacks: Option<PendingThreadRollback>,
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

    pub(crate) fn clear_listener(&mut self) {
        if let Some(cancel_tx) = self.cancel_tx.take() {
            let _ = cancel_tx.send(());
        }
        self.listener_command_tx = None;
        drop(self.pending_rollbacks.take());
        self.current_turn_history.reset();
        self.listener_thread = None;
        self.watch_registration = WatchRegistration::default();
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

    pub(crate) fn projected_active_turn_snapshot<'a>(
        &self,
        events: impl IntoIterator<Item = (&'a str, &'a EventMsg)>,
    ) -> Option<Turn> {
        let mut projected_history = self.current_turn_history.clone();
        for (_event_turn_id, event) in events {
            projected_history.handle_event(event);
            if matches!(event, EventMsg::TurnAborted(_) | EventMsg::TurnComplete(_))
                && !projected_history.has_active_turn()
            {
                projected_history.reset();
            }
        }
        projected_history.active_turn_snapshot()
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
    use codex_protocol::protocol::TurnCompleteEvent;
    use codex_protocol::protocol::TurnStartedEvent;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use pretty_assertions::assert_eq;

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
    fn projected_resume_overlay_does_not_advance_live_backlog_state() {
        let mut state = ThreadState::default();
        let a_start = EventMsg::TurnStarted(TurnStartedEvent {
            turn_id: "turn-a".to_string(),
            trace_id: None,
            started_at: Some(1),
            model_context_window: None,
            collaboration_mode_kind: Default::default(),
        });
        let a_complete = EventMsg::TurnComplete(TurnCompleteEvent {
            turn_id: "turn-a".to_string(),
            last_agent_message: None,
            completed_at: Some(2),
            duration_ms: Some(1_000),
            time_to_first_token_ms: None,
        });
        let b_start = EventMsg::TurnStarted(TurnStartedEvent {
            turn_id: "turn-b".to_string(),
            trace_id: None,
            started_at: Some(3),
            model_context_window: None,
            collaboration_mode_kind: Default::default(),
        });
        let backlog = [
            ("turn-a", &a_start),
            ("turn-a", &a_complete),
            ("turn-b", &b_start),
        ];

        let projected = state
            .projected_active_turn_snapshot(backlog)
            .expect("resume overlay should project the final active turn");
        assert_eq!(projected.id, "turn-b");
        assert!(state.active_turn_snapshot().is_none());

        state.track_current_turn_event("turn-a", &a_start);
        assert_eq!(
            state
                .active_turn_snapshot()
                .expect("A should be active while dispatching A start")
                .id,
            "turn-a"
        );
        state.track_current_turn_event("turn-a", &a_complete);
        assert!(state.active_turn_snapshot().is_none());
        state.track_current_turn_event("turn-b", &b_start);
        assert_eq!(
            state
                .active_turn_snapshot()
                .expect("B should be active while dispatching B start")
                .id,
            "turn-b"
        );
    }

    #[tokio::test]
    #[expect(
        clippy::await_holding_invalid_type,
        reason = "the test proves lease acquisition waits for the unload commit guard"
    )]
    async fn point_operation_lease_preserves_unsubscribe_and_blocks_idle_unload() {
        let manager = ThreadStateManager::new();
        let thread_id = ThreadId::new();
        let connection_id = ConnectionId(1);
        let pending_thread_unloads = Arc::new(Mutex::new(HashSet::new()));
        manager
            .connection_initialized(connection_id, ConnectionCapabilities::default())
            .await;
        manager
            .try_ensure_connection_subscribed(
                thread_id,
                connection_id,
                /*experimental_raw_events*/ false,
            )
            .await
            .expect("connection should be live");
        assert!(
            manager
                .unsubscribe_connection_from_thread(thread_id, connection_id)
                .await
        );
        assert!(!manager.has_subscribers(thread_id).await);

        let lease = match manager
            .acquire_point_operation_lease(&pending_thread_unloads, thread_id, connection_id)
            .await
        {
            ThreadPointOperationLeaseAcquireResult::Acquired { lease, .. } => lease,
            _ => panic!("point operation should accept a live unsubscribed connection"),
        };

        assert!(!manager.has_subscribers(thread_id).await);
        assert!(
            manager
                .subscribed_connection_ids(thread_id)
                .await
                .is_empty()
        );

        let mut point_operation_count = manager
            .subscribe_to_point_operation_count(thread_id)
            .await
            .expect("point-operation watcher");
        assert_eq!(*point_operation_count.borrow(), 1);
        let simulated_idle_unload = async {
            loop {
                let operation_in_flight = *point_operation_count.borrow_and_update() != 0;
                if !operation_in_flight {
                    break;
                }
                point_operation_count
                    .changed()
                    .await
                    .expect("point-operation watcher should remain open");
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        };
        tokio::pin!(simulated_idle_unload);
        assert!(
            tokio::time::timeout(
                std::time::Duration::from_millis(20),
                simulated_idle_unload.as_mut(),
            )
            .await
            .is_err(),
            "an idle unload must remain blocked while the point operation is leased"
        );
        drop(lease);
        tokio::time::timeout(std::time::Duration::from_secs(1), simulated_idle_unload)
            .await
            .expect("idle unload should proceed after the point operation completes");

        // Hold the same mutex across the unload path's final eligibility check and marker
        // commit. A racing acquisition cannot slip into that interval and must observe closing.
        let mut unload_commit = pending_thread_unloads.lock().await;
        let racing_acquire = manager.acquire_point_operation_lease(
            &pending_thread_unloads,
            thread_id,
            connection_id,
        );
        tokio::pin!(racing_acquire);
        assert!(
            tokio::time::timeout(
                std::time::Duration::from_millis(20),
                racing_acquire.as_mut(),
            )
            .await
            .is_err(),
            "lease acquisition must wait for the unload commit section"
        );
        unload_commit.insert(thread_id);
        drop(unload_commit);
        assert!(matches!(
            racing_acquire.await,
            ThreadPointOperationLeaseAcquireResult::ThreadClosing
        ));
        pending_thread_unloads.lock().await.clear();

        manager.remove_connection(connection_id).await;
        assert!(matches!(
            manager
                .acquire_point_operation_lease(&pending_thread_unloads, thread_id, connection_id,)
                .await,
            ThreadPointOperationLeaseAcquireResult::ConnectionClosed
        ));
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
    has_connections_watcher: watch::Sender<bool>,
    point_operation_count_watcher: watch::Sender<usize>,
}

impl Default for ThreadEntry {
    fn default() -> Self {
        Self {
            state: Arc::new(Mutex::new(ThreadState::default())),
            connection_ids: HashSet::new(),
            has_connections_watcher: watch::channel(false).0,
            point_operation_count_watcher: watch::channel(0).0,
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
}

#[derive(Clone, Copy, Default)]
pub(crate) struct ConnectionCapabilities {
    pub(crate) request_attestation: bool,
}

#[derive(Clone, Default)]
pub(crate) struct ThreadStateManager {
    state: Arc<Mutex<ThreadStateManagerInner>>,
    // Extension event sinks are synchronous, so they need an await-free way to
    // enqueue work on the active per-thread listener.
    listener_commands:
        Arc<StdMutex<HashMap<ThreadId, mpsc::UnboundedSender<ThreadListenerCommand>>>>,
}

pub(crate) struct ThreadPointOperationLease {
    point_operation_count_watcher: watch::Sender<usize>,
}

pub(crate) enum ThreadPointOperationLeaseAcquireResult {
    Acquired {
        thread_state: Arc<Mutex<ThreadState>>,
        lease: ThreadPointOperationLease,
    },
    ConnectionClosed,
    ThreadClosing,
}

impl Drop for ThreadPointOperationLease {
    fn drop(&mut self) {
        self.point_operation_count_watcher
            .send_modify(|count| *count = count.saturating_sub(1));
    }
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

    pub(crate) fn current_listener_command_tx(
        &self,
        thread_id: ThreadId,
    ) -> Option<mpsc::UnboundedSender<ThreadListenerCommand>> {
        self.listener_commands
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&thread_id)
            .cloned()
    }

    pub(crate) fn register_listener_command_tx(
        &self,
        thread_id: ThreadId,
        tx: mpsc::UnboundedSender<ThreadListenerCommand>,
    ) {
        self.listener_commands
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(thread_id, tx);
    }

    pub(crate) fn unregister_listener_command_tx(&self, thread_id: ThreadId) {
        self.listener_commands
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&thread_id);
    }

    pub(crate) async fn remove_thread_state(&self, thread_id: ThreadId) {
        let thread_state = {
            let mut state = self.state.lock().await;
            let thread_state = state
                .threads
                .remove(&thread_id)
                .map(|thread_entry| thread_entry.state);
            state.thread_ids_by_connection.retain(|_, thread_ids| {
                thread_ids.remove(&thread_id);
                !thread_ids.is_empty()
            });
            thread_state
        };
        self.unregister_listener_command_tx(thread_id);

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

    pub(crate) async fn clear_all_listeners(&self) {
        let thread_states = {
            let state = self.state.lock().await;
            state
                .threads
                .iter()
                .map(|(thread_id, thread_entry)| (*thread_id, thread_entry.state.clone()))
                .collect::<Vec<_>>()
        };

        for (thread_id, thread_state) in thread_states {
            self.unregister_listener_command_tx(thread_id);
            let mut thread_state = thread_state.lock().await;
            tracing::debug!(
                thread_id = %thread_id,
                listener_generation = thread_state.listener_generation,
                had_listener = thread_state.cancel_tx.is_some(),
                had_active_turn = thread_state.active_turn_snapshot().is_some(),
                "clearing thread listener during app-server shutdown"
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
                thread_entry.update_has_connections();
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

    pub(crate) async fn try_ensure_connection_subscribed(
        &self,
        thread_id: ThreadId,
        connection_id: ConnectionId,
        experimental_raw_events: bool,
    ) -> Option<Arc<Mutex<ThreadState>>> {
        let thread_state = {
            let mut state = self.state.lock().await;
            if !state.live_connections.contains_key(&connection_id) {
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

    #[expect(
        clippy::await_holding_invalid_type,
        reason = "lease acquisition must serialize with the unload marker commit"
    )]
    pub(crate) async fn acquire_point_operation_lease(
        &self,
        pending_thread_unloads: &Arc<Mutex<HashSet<ThreadId>>>,
        thread_id: ThreadId,
        connection_id: ConnectionId,
    ) -> ThreadPointOperationLeaseAcquireResult {
        // Serialize against the unload path's final eligibility check and closing-marker commit.
        // Once this guard is released, the watch-backed lease makes that final check fail.
        let pending_thread_unloads = pending_thread_unloads.lock().await;
        if pending_thread_unloads.contains(&thread_id) {
            return ThreadPointOperationLeaseAcquireResult::ThreadClosing;
        }
        let mut state = self.state.lock().await;
        if !state.live_connections.contains_key(&connection_id) {
            return ThreadPointOperationLeaseAcquireResult::ConnectionClosed;
        }
        let thread_entry = state.threads.entry(thread_id).or_default();
        thread_entry
            .point_operation_count_watcher
            .send_modify(|count| *count = count.saturating_add(1));
        ThreadPointOperationLeaseAcquireResult::Acquired {
            thread_state: thread_entry.state.clone(),
            lease: ThreadPointOperationLease {
                point_operation_count_watcher: thread_entry.point_operation_count_watcher.clone(),
            },
        }
    }

    pub(crate) async fn try_add_connection_to_thread(
        &self,
        thread_id: ThreadId,
        connection_id: ConnectionId,
    ) -> bool {
        let mut state = self.state.lock().await;
        if !state.live_connections.contains_key(&connection_id) {
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
                    thread_entry.update_has_connections();
                }
            }
            thread_ids
                .into_iter()
                .filter(|thread_id| {
                    state
                        .threads
                        .get(thread_id)
                        .is_some_and(|thread_entry| thread_entry.connection_ids.is_empty())
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

    pub(crate) async fn subscribe_to_point_operation_count(
        &self,
        thread_id: ThreadId,
    ) -> Option<watch::Receiver<usize>> {
        let state = self.state.lock().await;
        state
            .threads
            .get(&thread_id)
            .map(|thread_entry| thread_entry.point_operation_count_watcher.subscribe())
    }
}
