//! Serializes running-thread resume storage work with listener commands.

use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum BusyHistoryReadDisposition {
    ReturnBusy,
    RetryIdle,
    Conflict,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ListenerCommandTransition {
    None,
    ResumeStarted,
    ResumeFinished,
}

pub(super) enum ResumeInFlightEvent {
    DispatchImmediately(Event),
    Buffer(BufferedThreadEvent),
}

pub(super) const RESUME_EXEC_DELTA_REPLAY_MAX_EVENTS: usize = 10_000;
pub(super) const RESUME_EXEC_DELTA_REPLAY_TRUNCATION_MARKER: &[u8] =
    b"\n[output replay truncated while thread resume was in progress]\n";

#[derive(Default)]
pub(super) struct ResumeExecDeltaReplay {
    events: Vec<BufferedThreadEvent>,
    payload_bytes: usize,
    truncated: bool,
}

impl ResumeExecDeltaReplay {
    pub(super) fn retain(&mut self, event: &Event) {
        if self.truncated {
            return;
        }
        let EventMsg::ExecCommandOutputDelta(delta) = &event.msg else {
            debug_assert!(false, "only exec output deltas can enter resume replay");
            return;
        };
        let payload_limit = DEFAULT_OUTPUT_BYTES_CAP
            .saturating_sub(RESUME_EXEC_DELTA_REPLAY_TRUNCATION_MARKER.len());
        let event_limit = RESUME_EXEC_DELTA_REPLAY_MAX_EVENTS.saturating_sub(1);
        if self.events.len() < event_limit
            && self.payload_bytes.saturating_add(delta.chunk.len()) <= payload_limit
        {
            self.payload_bytes += delta.chunk.len();
            self.events
                .push(BufferedThreadEvent::from_resume_cut(event.clone()));
            return;
        }

        let mut marker = delta.clone();
        marker.chunk = RESUME_EXEC_DELTA_REPLAY_TRUNCATION_MARKER.to_vec();
        self.payload_bytes += marker.chunk.len();
        self.events
            .push(BufferedThreadEvent::from_resume_cut(Event {
                id: event.id.clone(),
                msg: EventMsg::ExecCommandOutputDelta(marker),
            }));
        self.truncated = true;
    }

    pub(super) fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    pub(super) fn events_mut(&mut self) -> &mut [BufferedThreadEvent] {
        self.events.as_mut_slice()
    }

    pub(super) fn into_events(self) -> Vec<BufferedThreadEvent> {
        self.events
    }

    #[cfg(test)]
    pub(super) fn payload_bytes(&self) -> usize {
        self.payload_bytes
    }

    #[cfg(test)]
    pub(super) fn is_truncated(&self) -> bool {
        self.truncated
    }
}

pub(super) fn route_resume_in_flight_event(
    event: Event,
    has_buffered_prefix: bool,
) -> ResumeInFlightEvent {
    if !has_buffered_prefix && matches!(&event.msg, EventMsg::ExecCommandOutputDelta(_)) {
        // Legacy exec can enqueue thousands of large output chunks without taking the event cut.
        // With no earlier queued event, its notification-only handler can safely stream those
        // chunks to existing subscribers while storage IO runs. Once any event is buffered, FIFO
        // takes priority so output cannot overtake its TurnStarted or ExecCommandBegin.
        ResumeInFlightEvent::DispatchImmediately(event)
    } else {
        ResumeInFlightEvent::Buffer(BufferedThreadEvent::from_resume_cut(event))
    }
}

pub(super) fn apply_listener_command_transition(
    resume_in_flight: &mut bool,
    transition: ListenerCommandTransition,
) {
    match transition {
        ListenerCommandTransition::ResumeStarted => *resume_in_flight = true,
        ListenerCommandTransition::ResumeFinished => *resume_in_flight = false,
        ListenerCommandTransition::None => {}
    }
}

pub(super) fn should_defer_listener_command(
    resume_in_flight: bool,
    listener_command: &ThreadListenerCommand,
) -> bool {
    resume_in_flight
        && !matches!(
            listener_command,
            ThreadListenerCommand::FinishThreadResumeResponse { .. }
        )
}

pub(super) async fn run_cancelable_resume_worker(
    cancellation_token: CancellationToken,
    resume: impl Future<Output = ()>,
) {
    tokio::select! {
        _ = cancellation_token.cancelled() => {}
        _ = resume => {}
    }
}

pub(super) fn classify_busy_history_read(
    attempt: usize,
    thread_became_idle: bool,
) -> BusyHistoryReadDisposition {
    if !thread_became_idle {
        BusyHistoryReadDisposition::ReturnBusy
    } else if attempt == 0 {
        BusyHistoryReadDisposition::RetryIdle
    } else {
        BusyHistoryReadDisposition::Conflict
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn handle_thread_listener_command(
    conversation_id: ThreadId,
    conversation: &Arc<CodexThread>,
    listener_task_context: &ListenerTaskContext,
    thread_state: &Arc<Mutex<ThreadState>>,
    listener_command_tx: &mpsc::UnboundedSender<ThreadListenerCommand>,
    resume_worker_cancel: &CancellationToken,
    buffered_resume_events: &mut Vec<BufferedThreadEvent>,
    resume_exec_delta_replay: &mut ResumeExecDeltaReplay,
    listener_command: ThreadListenerCommand,
) -> ListenerCommandTransition {
    match listener_command {
        ThreadListenerCommand::SendThreadResumeResponse {
            request: resume_request,
            completion_tx,
        } => {
            debug_assert!(buffered_resume_events.is_empty());
            debug_assert!(resume_exec_delta_replay.is_empty());
            if begin_pending_thread_resume_request(
                conversation_id,
                conversation,
                thread_state,
                &listener_task_context.outgoing,
                listener_command_tx,
                resume_worker_cancel,
                resume_request,
                completion_tx,
            )
            .await
            {
                ListenerCommandTransition::ResumeStarted
            } else {
                ListenerCommandTransition::None
            }
        }
        ThreadListenerCommand::FinishThreadResumeResponse {
            request: resume_request,
            completion_tx,
            history_result,
            release_event_cut_tx,
        } => {
            handle_pending_thread_resume_request(
                conversation_id,
                conversation,
                listener_task_context,
                thread_state,
                *resume_request,
                *history_result,
                std::mem::take(buffered_resume_events),
                std::mem::take(resume_exec_delta_replay),
                release_event_cut_tx,
            )
            .await;
            let _ = completion_tx.send(());
            ListenerCommandTransition::ResumeFinished
        }
        ThreadListenerCommand::EmitThreadGoalUpdated { turn_id, goal } => {
            listener_task_context
                .outgoing
                .send_server_notification(ServerNotification::ThreadGoalUpdated(
                    ThreadGoalUpdatedNotification {
                        thread_id: conversation_id.to_string(),
                        turn_id,
                        goal,
                    },
                ))
                .await;
            ListenerCommandTransition::None
        }
        ThreadListenerCommand::EmitThreadGoalCleared => {
            listener_task_context
                .outgoing
                .send_server_notification(ServerNotification::ThreadGoalCleared(
                    ThreadGoalClearedNotification {
                        thread_id: conversation_id.to_string(),
                    },
                ))
                .await;
            ListenerCommandTransition::None
        }
        ThreadListenerCommand::EmitThreadGoalSnapshot { state_db } => {
            send_thread_goal_snapshot_notification(
                &listener_task_context.outgoing,
                conversation_id,
                &state_db,
            )
            .await;
            ListenerCommandTransition::None
        }
        ThreadListenerCommand::ResolveServerRequest {
            request_id,
            completion_tx,
        } => {
            resolve_pending_server_request(
                conversation_id,
                &listener_task_context.thread_state_manager,
                &listener_task_context.outgoing,
                request_id,
            )
            .await;
            let _ = completion_tx.send(());
            ListenerCommandTransition::None
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn begin_pending_thread_resume_request(
    conversation_id: ThreadId,
    conversation: &Arc<CodexThread>,
    thread_state: &Arc<Mutex<ThreadState>>,
    outgoing: &Arc<OutgoingMessageSender>,
    listener_command_tx: &mpsc::UnboundedSender<ThreadListenerCommand>,
    resume_worker_cancel: &CancellationToken,
    request: Box<crate::thread_state::PendingThreadResumeRequest>,
    completion_tx: oneshot::Sender<()>,
) -> bool {
    if thread_state.lock().await.pending_rollbacks.is_some() {
        outgoing
            .send_error(
                request.request_id.clone(),
                invalid_request(format!(
                    "thread {conversation_id} has a rollback in progress; retry thread/resume after it finishes"
                )),
            )
            .await;
        let _ = completion_tx.send(());
        return false;
    }

    let conversation = Arc::clone(conversation);
    let listener_command_tx = listener_command_tx.clone();
    let resume_worker_cancel = resume_worker_cancel.child_token();
    tokio::spawn(async move {
        let resume = async move {
            // This worker owns all storage IO. Holding the event-delivery cut makes the loaded
            // history and queued-event boundary exact while the listener remains responsive to
            // cancellation and unload signals; the event queue is drained by Finish.
            let event_cut_guards = conversation
                .acquire_history_reconciliation_event_cut()
                .await;
            let history_result =
                read_pending_thread_resume_history(conversation_id, &conversation).await;
            let (release_event_cut_tx, release_event_cut_rx) = oneshot::channel();
            let command = ThreadListenerCommand::FinishThreadResumeResponse {
                request,
                completion_tx,
                history_result: Box::new(history_result),
                release_event_cut_tx,
            };
            if listener_command_tx.send(command).is_ok() {
                let _ = release_event_cut_rx.await;
            }
            drop(event_cut_guards);
        };
        run_cancelable_resume_worker(resume_worker_cancel, resume).await;
    });
    true
}

pub(super) async fn read_pending_thread_resume_history(
    conversation_id: ThreadId,
    conversation: &Arc<CodexThread>,
) -> Result<crate::thread_state::PreparedThreadResumeHistory, JSONRPCErrorError> {
    let mut attempt = 0;
    let (history_items, reconciliation_outcome) = loop {
        let snapshot = conversation.history_reconciliation_snapshot().await;
        // The event cut blocks new persisted event delivery, and the rollout guard blocks every
        // other append. Flush even while active so any queued event classified as
        // snapshot-represented is guaranteed to be present in this read.
        conversation.flush_rollout().await.map_err(|err| {
            internal_error(format!(
                "failed to flush thread {conversation_id} before resuming: {err}"
            ))
        })?;
        let history_items = conversation
            .load_history(/*include_archived*/ true)
            .await
            .map_err(super::thread_processor::thread_store_resume_read_error)?
            .items;
        let Some(snapshot) = snapshot else {
            let thread_became_idle = conversation
                .history_reconciliation_snapshot()
                .await
                .is_some();
            match classify_busy_history_read(attempt, thread_became_idle) {
                BusyHistoryReadDisposition::RetryIdle => {
                    // The loaded history may predate the terminal event. Retry through the idle
                    // flush/read path so the response includes the completed turn.
                    attempt += 1;
                    continue;
                }
                BusyHistoryReadDisposition::Conflict => {
                    break (history_items, ThreadHistoryReconciliationOutcome::Conflict);
                }
                BusyHistoryReadDisposition::ReturnBusy => {}
            }
            break (history_items, ThreadHistoryReconciliationOutcome::Busy);
        };
        let outcome = conversation
            .reconcile_persisted_history(snapshot, &history_items)
            .await;
        if attempt == 0
            && matches!(
                outcome,
                ThreadHistoryReconciliationOutcome::Conflict
                    | ThreadHistoryReconciliationOutcome::Incomplete
            )
        {
            attempt += 1;
            continue;
        }
        break (history_items, outcome);
    };
    match reconciliation_outcome {
        ThreadHistoryReconciliationOutcome::Incomplete => {
            return Err(invalid_request(format!(
                "thread {conversation_id} has an incomplete persisted turn; retry thread/resume after it finishes"
            )));
        }
        ThreadHistoryReconciliationOutcome::Conflict => {
            return Err(invalid_request(format!(
                "thread {conversation_id} changed while resuming; retry thread/resume"
            )));
        }
        ThreadHistoryReconciliationOutcome::Unchanged
        | ThreadHistoryReconciliationOutcome::Refreshed
        | ThreadHistoryReconciliationOutcome::Busy => {}
    }
    // Refresh metadata after the authoritative history/reconciliation read while the worker still
    // owns the history/event/rollout cut. Paginated-history threads cannot use read_thread with
    // history attached, so keep the canonical history load above and pair it with this fresh
    // metadata-only read in the same worker phase.
    let stored_thread = conversation
        .read_thread(
            /*include_archived*/ true, /*include_history*/ false,
        )
        .await
        .map_err(super::thread_processor::thread_store_resume_read_error)?;
    if stored_thread.archived_at.is_some() {
        return Err(invalid_request(format!(
            "session {conversation_id} is archived. Run `codex unarchive {conversation_id}` to unarchive it first."
        )));
    }
    Ok(crate::thread_state::PreparedThreadResumeHistory {
        stored_thread,
        history_items,
        reconciliation_outcome,
    })
}
