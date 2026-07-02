use super::*;
use crate::bespoke_event_handling::hook_prompt_thread_item;
use crate::bespoke_event_handling::maybe_emit_raw_response_item_completed;
use codex_protocol::config_types::MultiAgentMode;
use codex_protocol::protocol::Event;
use std::collections::VecDeque;
use std::future::Future;
use tokio::sync::mpsc;

pub(super) const THREAD_UNLOADING_DELAY: Duration = Duration::from_secs(30 * 60);

#[derive(Clone)]
pub(super) struct ListenerTaskContext {
    pub(super) thread_manager: Arc<ThreadManager>,
    pub(super) thread_state_manager: ThreadStateManager,
    pub(super) outgoing: Arc<OutgoingMessageSender>,
    pub(super) pending_thread_unloads: Arc<Mutex<HashSet<ThreadId>>>,
    pub(super) thread_watch_manager: ThreadWatchManager,
    pub(super) thread_list_state_permit: Arc<Semaphore>,
    pub(super) fallback_model_provider: String,
    pub(super) codex_home: PathBuf,
    pub(super) skills_watcher: Arc<SkillsWatcher>,
}

struct UnloadingState {
    delay: Duration,
    has_subscribers_rx: watch::Receiver<bool>,
    has_subscribers: (bool, Instant),
    point_operation_count_rx: watch::Receiver<usize>,
    thread_status_rx: watch::Receiver<ThreadStatus>,
    is_active: (bool, Instant),
}

impl UnloadingState {
    async fn new(
        listener_task_context: &ListenerTaskContext,
        thread_id: ThreadId,
        delay: Duration,
    ) -> Option<Self> {
        let has_subscribers_rx = listener_task_context
            .thread_state_manager
            .subscribe_to_has_connections(thread_id)
            .await?;
        let point_operation_count_rx = listener_task_context
            .thread_state_manager
            .subscribe_to_point_operation_count(thread_id)
            .await?;
        let thread_status_rx = listener_task_context
            .thread_watch_manager
            .subscribe(thread_id)
            .await?;
        let has_subscribers = (*has_subscribers_rx.borrow(), Instant::now());
        let is_active = (
            matches!(*thread_status_rx.borrow(), ThreadStatus::Active { .. }),
            Instant::now(),
        );
        Some(Self {
            delay,
            has_subscribers_rx,
            has_subscribers,
            point_operation_count_rx,
            thread_status_rx,
            is_active,
        })
    }

    fn unloading_target(&self) -> Option<Instant> {
        if *self.point_operation_count_rx.borrow() != 0 {
            return None;
        }
        match (self.has_subscribers, self.is_active) {
            ((false, has_no_subscribers_since), (false, is_inactive_since)) => {
                Some(std::cmp::max(has_no_subscribers_since, is_inactive_since) + self.delay)
            }
            _ => None,
        }
    }

    fn sync_receiver_values(&mut self) {
        let has_subscribers = *self.has_subscribers_rx.borrow();
        if self.has_subscribers.0 != has_subscribers {
            self.has_subscribers = (has_subscribers, Instant::now());
        }

        let is_active = matches!(*self.thread_status_rx.borrow(), ThreadStatus::Active { .. });
        if self.is_active.0 != is_active {
            self.is_active = (is_active, Instant::now());
        }
    }

    fn should_unload_now(&mut self) -> bool {
        self.sync_receiver_values();
        self.unloading_target()
            .is_some_and(|target| target <= Instant::now())
    }

    fn note_thread_activity_observed(&mut self) {
        if !self.is_active.0 {
            self.is_active = (false, Instant::now());
        }
    }

    async fn wait_for_unloading_trigger(&mut self) -> bool {
        loop {
            self.sync_receiver_values();
            let unloading_target = self.unloading_target();
            if let Some(target) = unloading_target
                && target <= Instant::now()
            {
                return true;
            }
            let unloading_sleep = async {
                if let Some(target) = unloading_target {
                    tokio::time::sleep_until(target.into()).await;
                } else {
                    futures::future::pending::<()>().await;
                }
            };
            tokio::select! {
                _ = unloading_sleep => return true,
                changed = self.has_subscribers_rx.changed() => {
                    if changed.is_err() {
                        return false;
                    }
                    self.sync_receiver_values();
                },
                changed = self.point_operation_count_rx.changed() => {
                    if changed.is_err() {
                        return false;
                    }
                },
                changed = self.thread_status_rx.changed() => {
                    if changed.is_err() {
                        return false;
                    }
                    self.sync_receiver_values();
                },
            }
        }
    }
}

pub(super) enum ThreadShutdownResult {
    Complete,
    SubmitFailed,
    TimedOut,
}

pub(super) enum EnsureConversationListenerResult {
    Attached,
    ConnectionClosed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ResumeEventCoverage {
    pub(super) represented_in_resume_snapshot: bool,
    pub(super) request_live_for_resumed_connection: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ResumePayloadMode {
    Full,
    Redacted,
}

impl ResumePayloadMode {
    fn is_redacted(self) -> bool {
        matches!(self, Self::Redacted)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct BufferedRawResponseRouting {
    pub(super) event_coverage: ResumeEventCoverage,
    pub(super) raw_events_enabled: bool,
    pub(super) resume_payload_mode: ResumePayloadMode,
}

#[expect(
    clippy::await_holding_invalid_type,
    reason = "listener subscription must be serialized against pending unloads"
)]
pub(super) async fn ensure_conversation_listener(
    listener_task_context: ListenerTaskContext,
    conversation_id: ThreadId,
    connection_id: ConnectionId,
    raw_events_enabled: bool,
) -> Result<EnsureConversationListenerResult, JSONRPCErrorError> {
    let conversation = match listener_task_context
        .thread_manager
        .get_thread(conversation_id)
        .await
    {
        Ok(conv) => conv,
        Err(_) => {
            return Err(invalid_request(format!(
                "thread not found: {conversation_id}"
            )));
        }
    };
    let thread_state = {
        let pending_thread_unloads = listener_task_context.pending_thread_unloads.lock().await;
        if pending_thread_unloads.contains(&conversation_id) {
            return Err(invalid_request(format!(
                "thread {conversation_id} is closing; retry after the thread is closed"
            )));
        }
        let Some(thread_state) = listener_task_context
            .thread_state_manager
            .try_ensure_connection_subscribed(conversation_id, connection_id, raw_events_enabled)
            .await
        else {
            return Ok(EnsureConversationListenerResult::ConnectionClosed);
        };
        thread_state
    };
    if let Err(error) = ensure_listener_task_running(
        listener_task_context.clone(),
        conversation_id,
        conversation,
        thread_state,
    )
    .await
    {
        let _ = listener_task_context
            .thread_state_manager
            .unsubscribe_connection_from_thread(conversation_id, connection_id)
            .await;
        return Err(error);
    }
    Ok(EnsureConversationListenerResult::Attached)
}

pub(super) fn log_listener_attach_result(
    result: Result<EnsureConversationListenerResult, JSONRPCErrorError>,
    thread_id: ThreadId,
    connection_id: ConnectionId,
    thread_kind: &'static str,
) {
    match result {
        Ok(EnsureConversationListenerResult::Attached) => {}
        Ok(EnsureConversationListenerResult::ConnectionClosed) => {
            tracing::debug!(
                thread_id = %thread_id,
                connection_id = ?connection_id,
                "skipping auto-attach for closed connection"
            );
        }
        Err(err) => {
            tracing::warn!(
                "failed to attach listener for {thread_kind} {thread_id}: {message}",
                message = err.message
            );
        }
    }
}

pub(super) async fn ensure_listener_task_running(
    listener_task_context: ListenerTaskContext,
    conversation_id: ThreadId,
    conversation: Arc<CodexThread>,
    thread_state: Arc<Mutex<ThreadState>>,
) -> Result<(), JSONRPCErrorError> {
    let (cancel_tx, mut cancel_rx) = oneshot::channel();
    let Some(mut unloading_state) = UnloadingState::new(
        &listener_task_context,
        conversation_id,
        THREAD_UNLOADING_DELAY,
    )
    .await
    else {
        return Err(invalid_request(format!(
            "thread {conversation_id} is closing; retry after the thread is closed"
        )));
    };
    let config = conversation.config().await;
    let environments = conversation.environment_selections().await;
    let watch_registration = listener_task_context
        .skills_watcher
        .register_thread_config(
            config.as_ref(),
            listener_task_context.thread_manager.as_ref(),
            &environments,
        )
        .await;
    let thread_settings_baseline =
        thread_settings_from_config_snapshot(&conversation.config_snapshot().await);
    let (mut listener_command_rx, listener_command_tx, listener_generation) = {
        let mut thread_state = thread_state.lock().await;
        if thread_state.listener_matches(&conversation) {
            return Ok(());
        }
        let (listener_command_rx, listener_generation) = thread_state.set_listener(
            cancel_tx,
            &conversation,
            watch_registration,
            thread_settings_baseline,
        );
        let Some(listener_command_tx) = thread_state.listener_command_tx() else {
            tracing::warn!(
                "thread listener command sender missing immediately after listener registration"
            );
            return Ok(());
        };
        listener_task_context
            .thread_state_manager
            .register_listener_command_tx(conversation_id, listener_command_tx.clone());
        (
            listener_command_rx,
            listener_command_tx,
            listener_generation,
        )
    };
    let ListenerTaskContext {
        outgoing,
        thread_manager,
        thread_state_manager,
        pending_thread_unloads,
        thread_watch_manager,
        thread_list_state_permit,
        fallback_model_provider,
        codex_home,
        ..
    } = listener_task_context;
    let outgoing_for_task = Arc::clone(&outgoing);
    tokio::spawn(async move {
        let resume_worker_cancel = CancellationToken::new();
        let _resume_worker_cancel_guard = resume_worker_cancel.clone().drop_guard();
        let mut resume_in_flight = false;
        let mut buffered_resume_events = Vec::new();
        let mut resume_exec_delta_replay = ResumeExecDeltaReplay::default();
        let mut deferred_listener_commands = VecDeque::new();
        loop {
            if !resume_in_flight {
                while let Some(listener_command) = deferred_listener_commands.pop_front() {
                    let transition = handle_thread_listener_command(
                        conversation_id,
                        &conversation,
                        codex_home.as_path(),
                        &thread_manager,
                        &thread_state_manager,
                        &thread_state,
                        &thread_watch_manager,
                        &thread_list_state_permit,
                        &fallback_model_provider,
                        &outgoing_for_task,
                        &pending_thread_unloads,
                        &listener_command_tx,
                        &resume_worker_cancel,
                        &mut buffered_resume_events,
                        &mut resume_exec_delta_replay,
                        listener_command,
                    )
                    .await;
                    apply_listener_command_transition(&mut resume_in_flight, transition);
                    if resume_in_flight {
                        break;
                    }
                }
            }
            tokio::select! {
                biased;
                _ = &mut cancel_rx => {
                    // Listener was superseded or the thread is being torn down.
                    break;
                }
                listener_command = listener_command_rx.recv() => {
                    let Some(listener_command) = listener_command else {
                        break;
                    };
                    if should_defer_listener_command(resume_in_flight, &listener_command) {
                        deferred_listener_commands.push_back(listener_command);
                        continue;
                    }
                    let transition = handle_thread_listener_command(
                        conversation_id,
                        &conversation,
                        codex_home.as_path(),
                        &thread_manager,
                        &thread_state_manager,
                        &thread_state,
                        &thread_watch_manager,
                        &thread_list_state_permit,
                        &fallback_model_provider,
                        &outgoing_for_task,
                        &pending_thread_unloads,
                        &listener_command_tx,
                        &resume_worker_cancel,
                        &mut buffered_resume_events,
                        &mut resume_exec_delta_replay,
                        listener_command,
                    )
                    .await;
                    apply_listener_command_transition(&mut resume_in_flight, transition);
                }
                event = conversation.next_event() => {
                    let event = match event {
                        Ok(event) => event,
                        Err(err) => {
                            tracing::warn!("thread.next_event() failed with: {err}");
                            break;
                        }
                    };

                    let event = if resume_in_flight {
                        match route_resume_in_flight_event(
                            event,
                            /*has_buffered_prefix*/ !buffered_resume_events.is_empty(),
                        ) {
                            ResumeInFlightEvent::DispatchImmediately(event) => {
                                resume_exec_delta_replay.retain(&event);
                                event
                            }
                            ResumeInFlightEvent::Buffer(buffered) => {
                                buffered_resume_events.push(buffered);
                                continue;
                            }
                        }
                    } else {
                        event
                    };

                    let raw_events_enabled =
                        track_thread_event(&thread_state, &event).await;
                    let subscribed_connection_ids = thread_state_manager
                        .subscribed_connection_ids(conversation_id)
                        .await;
                    dispatch_thread_event(
                        event,
                        conversation_id,
                        &conversation,
                        &thread_manager,
                        &thread_state,
                        &thread_watch_manager,
                        &thread_list_state_permit,
                        &fallback_model_provider,
                        &outgoing_for_task,
                        subscribed_connection_ids,
                        /*item_lifecycle_connection_ids*/ None,
                        raw_events_enabled,
                    )
                    .await;
                }
                unloading_watchers_open = unloading_state.wait_for_unloading_trigger() => {
                    if !unloading_watchers_open {
                        break;
                    }
                    if !unloading_state.should_unload_now() {
                        continue;
                    }
                    if matches!(conversation.agent_status().await, AgentStatus::Running) {
                        unloading_state.note_thread_activity_observed();
                        continue;
                    }
                    {
                        let mut pending_thread_unloads = pending_thread_unloads.lock().await;
                        if pending_thread_unloads.contains(&conversation_id) {
                            continue;
                        }
                        if !unloading_state.should_unload_now() {
                            continue;
                        }
                        pending_thread_unloads.insert(conversation_id);
                    }
                    unload_thread_without_subscribers(
                        thread_manager.clone(),
                        outgoing_for_task.clone(),
                        pending_thread_unloads.clone(),
                        thread_state_manager.clone(),
                        thread_watch_manager.clone(),
                        conversation_id,
                        conversation.clone(),
                    )
                    .await;
                    break;
                }
            }
        }

        resume_worker_cancel.cancel();

        let mut thread_state = thread_state.lock().await;
        if thread_state.listener_generation == listener_generation {
            thread_state_manager.unregister_listener_command_tx(conversation_id);
            thread_state.clear_listener();
        }
    });
    Ok(())
}

async fn track_thread_event(thread_state: &Arc<Mutex<ThreadState>>, event: &Event) -> bool {
    // Track before emitting typed translations so thread-local state, including the active-turn
    // overlay and raw event opt-in, stays synchronized with the core conversation.
    let mut thread_state = thread_state.lock().await;
    thread_state.track_current_turn_event(&event.id, &event.msg);
    thread_state.experimental_raw_events
}

#[allow(clippy::too_many_arguments)]
async fn dispatch_thread_event(
    event: Event,
    conversation_id: ThreadId,
    conversation: &Arc<CodexThread>,
    thread_manager: &Arc<ThreadManager>,
    thread_state: &Arc<Mutex<ThreadState>>,
    thread_watch_manager: &ThreadWatchManager,
    thread_list_state_permit: &Arc<Semaphore>,
    fallback_model_provider: &str,
    outgoing: &Arc<OutgoingMessageSender>,
    subscribed_connection_ids: Vec<ConnectionId>,
    item_lifecycle_connection_ids: Option<Vec<ConnectionId>>,
    raw_events_enabled: bool,
) {
    let thread_outgoing = ThreadScopedOutgoingMessageSender::new(
        Arc::clone(outgoing),
        subscribed_connection_ids,
        conversation_id,
    );

    if let EventMsg::RawResponseItem(raw_response_item_event) = &event.msg
        && !raw_events_enabled
    {
        maybe_emit_hook_prompt_item_completed(
            conversation_id,
            &event.id,
            &raw_response_item_event.item,
            &thread_outgoing,
        )
        .await;
        return;
    }

    let item_lifecycle_outgoing = item_lifecycle_connection_ids.map(|connection_ids| {
        ThreadScopedOutgoingMessageSender::new(
            Arc::clone(outgoing),
            connection_ids,
            conversation_id,
        )
    });
    if let Some(item_lifecycle_outgoing) = item_lifecycle_outgoing {
        apply_bespoke_event_handling_with_item_lifecycle_outgoing(
            event,
            conversation_id,
            Arc::clone(conversation),
            Arc::clone(thread_manager),
            thread_outgoing,
            Arc::clone(thread_state),
            thread_watch_manager.clone(),
            Arc::clone(thread_list_state_permit),
            fallback_model_provider.to_string(),
            Some(item_lifecycle_outgoing),
        )
        .await;
    } else {
        apply_bespoke_event_handling(
            event,
            conversation_id,
            Arc::clone(conversation),
            Arc::clone(thread_manager),
            thread_outgoing,
            Arc::clone(thread_state),
            thread_watch_manager.clone(),
            Arc::clone(thread_list_state_permit),
            fallback_model_provider.to_string(),
        )
        .await;
    }
}

#[path = "thread_lifecycle/resume_event_coverage.rs"]
mod resume_event_coverage;
use resume_event_coverage::*;

#[path = "thread_lifecycle/resume_event_dispatch.rs"]
mod resume_event_dispatch;
use resume_event_dispatch::*;

pub(super) async fn wait_for_thread_shutdown(thread: &Arc<CodexThread>) -> ThreadShutdownResult {
    match tokio::time::timeout(Duration::from_secs(10), thread.shutdown_and_wait()).await {
        Ok(Ok(())) => ThreadShutdownResult::Complete,
        Ok(Err(_)) => ThreadShutdownResult::SubmitFailed,
        Err(_) => ThreadShutdownResult::TimedOut,
    }
}

pub(super) async fn unload_thread_without_subscribers(
    thread_manager: Arc<ThreadManager>,
    outgoing: Arc<OutgoingMessageSender>,
    pending_thread_unloads: Arc<Mutex<HashSet<ThreadId>>>,
    thread_state_manager: ThreadStateManager,
    thread_watch_manager: ThreadWatchManager,
    thread_id: ThreadId,
    thread: Arc<CodexThread>,
) {
    info!("thread {thread_id} has no subscribers and is idle; shutting down");

    // Any pending app-server -> client requests for this thread can no longer be
    // answered; cancel their callbacks before shutdown/unload.
    outgoing
        .cancel_requests_for_thread(thread_id, /*error*/ None)
        .await;
    thread_state_manager.remove_thread_state(thread_id).await;

    tokio::spawn(async move {
        match wait_for_thread_shutdown(&thread).await {
            ThreadShutdownResult::Complete => {
                if thread_manager.remove_thread(&thread_id).await.is_none() {
                    info!("thread {thread_id} was already removed before teardown finalized");
                    thread_watch_manager
                        .remove_thread(&thread_id.to_string())
                        .await;
                    pending_thread_unloads.lock().await.remove(&thread_id);
                    return;
                }
                thread_watch_manager
                    .remove_thread(&thread_id.to_string())
                    .await;
                let notification = ThreadClosedNotification {
                    thread_id: thread_id.to_string(),
                };
                outgoing
                    .send_server_notification(ServerNotification::ThreadClosed(notification))
                    .await;
                pending_thread_unloads.lock().await.remove(&thread_id);
            }
            ThreadShutdownResult::SubmitFailed => {
                pending_thread_unloads.lock().await.remove(&thread_id);
                warn!("failed to submit Shutdown to thread {thread_id}");
            }
            ThreadShutdownResult::TimedOut => {
                pending_thread_unloads.lock().await.remove(&thread_id);
                warn!("thread {thread_id} shutdown timed out; leaving thread loaded");
            }
        }
    });
}

#[path = "thread_lifecycle/resume_response.rs"]
mod resume_response;
use resume_response::*;

#[path = "thread_lifecycle/resume_worker.rs"]
mod resume_worker;
use resume_worker::*;
pub(super) async fn send_thread_goal_snapshot_notification(
    outgoing: &Arc<OutgoingMessageSender>,
    thread_id: ThreadId,
    state_db: &StateDbHandle,
) {
    match state_db.thread_goals().get_thread_goal(thread_id).await {
        Ok(Some(goal)) => {
            outgoing
                .send_server_notification(ServerNotification::ThreadGoalUpdated(
                    ThreadGoalUpdatedNotification {
                        thread_id: thread_id.to_string(),
                        turn_id: None,
                        goal: api_thread_goal_from_state(goal),
                    },
                ))
                .await;
        }
        Ok(None) => {
            outgoing
                .send_server_notification(ServerNotification::ThreadGoalCleared(
                    ThreadGoalClearedNotification {
                        thread_id: thread_id.to_string(),
                    },
                ))
                .await;
        }
        Err(err) => {
            tracing::warn!(
                thread_id = %thread_id,
                "failed to read thread goal for resume snapshot: {err}"
            );
        }
    }
}

pub(crate) fn populate_thread_turns_from_history(
    thread: &mut Thread,
    items: &[RolloutItem],
    active_turn: Option<&Turn>,
) {
    let mut turns = build_api_turns_from_rollout_items(items);
    if let Some(active_turn) = active_turn {
        merge_turn_history_with_active_turn(&mut turns, active_turn.clone());
    }
    thread.turns = turns;
}

pub(super) async fn resolve_pending_server_request(
    conversation_id: ThreadId,
    thread_state_manager: &ThreadStateManager,
    outgoing: &Arc<OutgoingMessageSender>,
    request_id: RequestId,
) {
    let thread_id = conversation_id.to_string();
    let subscribed_connection_ids = thread_state_manager
        .subscribed_connection_ids(conversation_id)
        .await;
    let outgoing = ThreadScopedOutgoingMessageSender::new(
        outgoing.clone(),
        subscribed_connection_ids,
        conversation_id,
    );
    outgoing
        .send_server_notification(ServerNotification::ServerRequestResolved(
            ServerRequestResolvedNotification {
                thread_id,
                request_id,
            },
        ))
        .await;
}

pub(super) fn merge_turn_history_with_active_turn(turns: &mut Vec<Turn>, active_turn: Turn) {
    turns.retain(|turn| turn.id != active_turn.id);
    turns.push(active_turn);
}

pub(super) fn set_thread_status_and_interrupt_stale_turns(
    thread: &mut Thread,
    loaded_status: ThreadStatus,
    has_live_in_progress_turn: bool,
) {
    let status = resolve_thread_status(loaded_status, has_live_in_progress_turn);
    if !matches!(status, ThreadStatus::Active { .. }) {
        for turn in &mut thread.turns {
            if matches!(turn.status, TurnStatus::InProgress) {
                turn.status = TurnStatus::Interrupted;
            }
        }
    }
    thread.status = status;
}

#[cfg(test)]
#[path = "thread_lifecycle_tests.rs"]
mod tests;
