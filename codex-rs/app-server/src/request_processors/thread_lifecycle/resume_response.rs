//! Composes a loaded-thread resume response at the exact history/event cut.

use super::*;

#[allow(clippy::too_many_arguments)]
#[expect(
    clippy::await_holding_invalid_type,
    reason = "running-thread resume subscription must be serialized against pending unloads"
)]
pub(super) async fn handle_pending_thread_resume_request(
    conversation_id: ThreadId,
    conversation: &Arc<CodexThread>,
    listener_task_context: &ListenerTaskContext,
    thread_state: &Arc<Mutex<ThreadState>>,
    pending: crate::thread_state::PendingThreadResumeRequest,
    history_result: Result<crate::thread_state::PreparedThreadResumeHistory, JSONRPCErrorError>,
    mut buffered_events: Vec<BufferedThreadEvent>,
    mut exec_delta_replay: ResumeExecDeltaReplay,
    release_event_cut_tx: oneshot::Sender<()>,
) {
    let ListenerTaskContext {
        thread_state_manager,
        outgoing,
        pending_thread_unloads,
        thread_watch_manager,
        ..
    } = listener_task_context;
    let pre_cut_connection_ids = thread_state_manager
        .subscribed_connection_ids(conversation_id)
        .await;
    // Drain exactly the queue depth observed at the cut. A direct legacy-exec producer may keep
    // appending output deltas, so an open-ended try-receive loop could otherwise starve the
    // response forever. Later events remain queued for the listener after the cut is released.
    let queued_event_count = conversation.pending_event_count();
    for _ in 0..queued_event_count {
        let event = match conversation.try_next_event() {
            Ok(Some(event)) => event,
            Ok(None) => break,
            Err(err) => {
                drop(release_event_cut_tx);
                outgoing
                    .send_error(
                        pending.request_id.clone(),
                        internal_error(format!(
                            "failed to finish resuming thread {conversation_id}: {err}"
                        )),
                    )
                    .await;
                dispatch_buffered_thread_events(
                    buffered_events,
                    &pre_cut_connection_ids,
                    None,
                    conversation_id,
                    conversation,
                    listener_task_context,
                    thread_state,
                    ResumePayloadMode::Full,
                )
                .await;
                return;
            }
        };
        match route_resume_in_flight_event(
            event,
            /*has_buffered_prefix*/ !buffered_events.is_empty(),
        ) {
            ResumeInFlightEvent::DispatchImmediately(event) => {
                exec_delta_replay.retain(&event);
                let raw_events_enabled = track_thread_event(thread_state, &event).await;
                dispatch_thread_event(
                    event,
                    conversation_id,
                    conversation,
                    listener_task_context,
                    thread_state,
                    pre_cut_connection_ids.clone(),
                    /*item_lifecycle_connection_ids*/ None,
                    raw_events_enabled,
                )
                .await;
            }
            ResumeInFlightEvent::Buffer(buffered) => buffered_events.push(buffered),
        }
    }
    let crate::thread_state::PreparedThreadResumeHistory {
        stored_thread,
        history_items,
        reconciliation_outcome,
    } = match history_result {
        Ok(history) => history,
        Err(error) => {
            drop(release_event_cut_tx);
            outgoing.send_error(pending.request_id, error).await;
            dispatch_buffered_thread_events(
                buffered_events,
                &pre_cut_connection_ids,
                None,
                conversation_id,
                conversation,
                listener_task_context,
                thread_state,
                ResumePayloadMode::Full,
            )
            .await;
            return;
        }
    };
    let pre_cut_pending_requests_survive = project_buffered_request_liveness(&mut buffered_events);
    let active_turn = {
        let state = thread_state.lock().await;
        matches!(
            reconciliation_outcome,
            ThreadHistoryReconciliationOutcome::Busy
        )
        .then(|| {
            state.projected_active_turn_snapshot(
                buffered_events
                    .iter()
                    .map(|buffered| (buffered.event.id.as_str(), &buffered.event.msg)),
            )
        })
        .flatten()
    };
    tracing::debug!(
        thread_id = %conversation_id,
        request_id = ?pending.request_id,
        active_turn_present = active_turn.is_some(),
        active_turn_id = ?active_turn.as_ref().map(|turn| turn.id.as_str()),
        active_turn_status = ?active_turn.as_ref().map(|turn| &turn.status),
        "composing running thread resume response"
    );
    let has_live_in_progress_turn =
        matches!(conversation.agent_status().await, AgentStatus::Running)
            || active_turn
                .as_ref()
                .is_some_and(|turn| matches!(turn.status, TurnStatus::InProgress));

    let request_id = pending.request_id;
    let connection_id = request_id.connection_id;
    let was_subscribed = pre_cut_connection_ids.contains(&connection_id);
    let mut thread = super::thread_processor::thread_from_stored_thread(
        stored_thread,
        pending.config_snapshot.model_provider_id.as_str(),
        pending.config_snapshot.cwd(),
    )
    .0;
    thread.thread_source = pending
        .config_snapshot
        .thread_source
        .clone()
        .map(Into::into);
    thread.session_id = conversation.session_configured().session_id.to_string();
    if pending.include_turns {
        populate_thread_turns_from_history(&mut thread, &history_items, active_turn.as_ref());
    }

    let loaded_thread_status = thread_watch_manager
        .loaded_status_for_thread(&thread.id)
        .await;
    let thread_status = project_thread_status_after_buffered_events(
        loaded_thread_status,
        has_live_in_progress_turn,
        buffered_events.iter().map(|buffered| &buffered.event.msg),
    );

    set_thread_status_and_interrupt_stale_turns(
        &mut thread,
        thread_status,
        has_live_in_progress_turn,
    );
    let token_usage_thread = pending.include_turns.then(|| thread.clone());
    let mut initial_turns_page = if let Some(params) = pending.initial_turns_page.as_ref() {
        match super::thread_processor::build_thread_resume_initial_turns_page(
            &history_items,
            thread.status.clone(),
            has_live_in_progress_turn,
            active_turn,
            params,
        ) {
            Ok(page) => Some(page),
            Err(error) => {
                drop(release_event_cut_tx);
                outgoing.send_error(request_id, error).await;
                dispatch_buffered_thread_events(
                    buffered_events,
                    &pre_cut_connection_ids,
                    None,
                    conversation_id,
                    conversation,
                    listener_task_context,
                    thread_state,
                    ResumePayloadMode::Full,
                )
                .await;
                return;
            }
        }
    } else {
        None
    };
    let resume_payload_mode = if pending.redact_resume_payloads {
        ResumePayloadMode::Redacted
    } else {
        ResumePayloadMode::Full
    };
    let reconciled_token_usage = latest_token_usage_info_from_rollout_items(&history_items);
    let reconciled_token_usage_turn_id = token_usage_thread.as_ref().and_then(|thread| {
        latest_token_usage_turn_id_from_rollout_items(&history_items, thread.turns.as_slice())
    });
    let replay_reconciled_token_usage = should_replay_reconciled_token_usage(
        &buffered_events,
        reconciled_token_usage.as_ref(),
        reconciled_token_usage_turn_id.as_deref(),
    );
    if !buffered_events.is_empty() || !exec_delta_replay.is_empty() {
        let mut item_coverage =
            ResumePayloadItemCoverage::new(thread.turns.as_slice(), initial_turns_page.as_ref());
        // Classify in reverse so a final canonical item claims its durable response occurrence
        // before the earlier ItemStarted for the same canonical id asks to reuse that claim.
        // Direct-dispatched exec deltas always precede the ordinary buffered prefix, so visit
        // that prefix first in reverse order. A later command completion can then suppress
        // replayed output already represented by its final snapshot item.
        for buffered in buffered_events
            .iter_mut()
            .rev()
            .chain(exec_delta_replay.events_mut().iter_mut().rev())
        {
            buffered.represented_in_resume_snapshot =
                buffered_event_is_represented_in_resume_payload(
                    buffered,
                    thread.turns.as_slice(),
                    initial_turns_page.as_ref(),
                    &mut item_coverage,
                    resume_payload_mode,
                );
        }
    }
    if resume_payload_mode.is_redacted() {
        redact_thread_resume_payloads(&mut thread.turns);
        if let Some(initial_turns_page) = initial_turns_page.as_mut() {
            redact_thread_resume_payloads(&mut initial_turns_page.data);
        }
    }

    {
        let pending_thread_unloads = pending_thread_unloads.lock().await;
        if pending_thread_unloads.contains(&conversation_id) {
            drop(pending_thread_unloads);
            drop(release_event_cut_tx);
            outgoing
                .send_error(
                    request_id,
                    invalid_request(format!(
                        "thread {conversation_id} is closing; retry thread/resume after the thread is closed"
                    )),
                )
                .await;
            dispatch_buffered_thread_events(
                buffered_events,
                &pre_cut_connection_ids,
                None,
                conversation_id,
                conversation,
                listener_task_context,
                thread_state,
                ResumePayloadMode::Full,
            )
            .await;
            return;
        }
        if !thread_state_manager
            .try_add_connection_to_thread(conversation_id, connection_id)
            .await
        {
            tracing::debug!(
                thread_id = %conversation_id,
                connection_id = ?connection_id,
                "skipping running thread resume for closed connection"
            );
            drop(pending_thread_unloads);
            drop(release_event_cut_tx);
            dispatch_buffered_thread_events(
                buffered_events,
                &pre_cut_connection_ids,
                None,
                conversation_id,
                conversation,
                listener_task_context,
                thread_state,
                ResumePayloadMode::Full,
            )
            .await;
            return;
        }
        // Client compatibility settings are live thread state. Apply them only after every
        // resume validation succeeds so a rejected resume cannot affect attached clients.
        if let Err(error) =
            super::thread_processor::ThreadRequestProcessor::set_app_server_client_info(
                conversation.as_ref(),
                pending.app_server_client_name,
                pending.app_server_client_version,
            )
            .await
        {
            if !was_subscribed {
                thread_state_manager
                    .unsubscribe_connection_from_thread(conversation_id, connection_id)
                    .await;
            }
            drop(pending_thread_unloads);
            drop(release_event_cut_tx);
            outgoing.send_error(request_id, error).await;
            dispatch_buffered_thread_events(
                buffered_events,
                &pre_cut_connection_ids,
                None,
                conversation_id,
                conversation,
                listener_task_context,
                thread_state,
                ResumePayloadMode::Full,
            )
            .await;
            return;
        }
    }

    let config_snapshot = pending.config_snapshot;
    let cwd = config_snapshot.cwd().clone();
    let ThreadConfigSnapshot {
        model,
        model_provider_id,
        service_tier,
        approval_policy,
        approvals_reviewer,
        permission_profile,
        active_permission_profile,
        workspace_roots,
        reasoning_effort,
        originator,
        ..
    } = config_snapshot;
    let instruction_sources = pending.instruction_sources;
    let sandbox = thread_response_sandbox_policy(&permission_profile, cwd.as_path());
    let active_permission_profile =
        thread_response_active_permission_profile(active_permission_profile);
    let session_id = conversation.session_configured().session_id.to_string();
    thread.session_id = session_id;

    let response = ThreadResumeResponse {
        thread,
        model,
        model_provider: model_provider_id,
        service_tier,
        cwd,
        runtime_workspace_roots: workspace_roots,
        instruction_sources,
        approval_policy: approval_policy.into(),
        approvals_reviewer: approvals_reviewer.into(),
        sandbox,
        active_permission_profile,
        reasoning_effort,
        multi_agent_mode: MultiAgentMode::ExplicitRequestOnly,
        initial_turns_page,
    };
    outgoing
        .send_response_with_thread_originator(request_id, response, originator)
        .await;
    // Replay only requests that predate the resume cut. Buffered request-producing events have
    // not run their bespoke handlers yet, so their requests will be created exactly once below
    // and can retain companion notifications such as item/started for the joining connection.
    if pre_cut_pending_requests_survive {
        outgoing
            .replay_requests_to_connection_for_thread(connection_id, conversation_id)
            .await;
    }
    drop(release_event_cut_tx);
    if !was_subscribed {
        dispatch_replayed_exec_deltas_to_connection(
            exec_delta_replay,
            connection_id,
            conversation_id,
            outgoing,
        )
        .await;
    }
    dispatch_buffered_thread_events(
        buffered_events,
        &pre_cut_connection_ids,
        Some(connection_id),
        conversation_id,
        conversation,
        listener_task_context,
        thread_state,
        resume_payload_mode,
    )
    .await;
    // Match cold resume: metadata-only resume should attach the listener without
    // paying the cost of turn reconstruction for historical usage replay.
    if let Some(token_usage_thread) = token_usage_thread
        && replay_reconciled_token_usage
        && let Some(reconciled_token_usage) = reconciled_token_usage
    {
        // Rejoining a loaded thread has the same UI contract as a cold resume, but
        // uses the live conversation state instead of reconstructing a new session.
        send_thread_token_usage_snapshot_to_connection(
            outgoing,
            connection_id,
            conversation_id,
            &token_usage_thread,
            reconciled_token_usage,
            reconciled_token_usage_turn_id,
        )
        .await;
    }
    if pending.emit_thread_goal_update {
        if let Some(state_db) = pending.thread_goal_state_db {
            send_thread_goal_snapshot_notification(outgoing, conversation_id, &state_db).await;
        } else {
            tracing::warn!(
                thread_id = %conversation_id,
                "state db unavailable when reading thread goal for running thread resume"
            );
        }
    }
    // App-server owns resume response and snapshot ordering, so wait until every replay completes
    // before letting extensions react to the idle thread.
    if pending.emit_thread_goal_update {
        conversation.emit_thread_idle_lifecycle_if_idle().await;
    }
}

pub(super) fn project_thread_status_after_buffered_events<'a>(
    loaded_status: ThreadStatus,
    has_live_in_progress_turn: bool,
    events: impl IntoIterator<Item = &'a EventMsg>,
) -> ThreadStatus {
    let latest_turn_transition_is_terminal = events
        .into_iter()
        .fold(None, |projected, event| match event {
            EventMsg::TurnStarted(_) => Some(false),
            EventMsg::TurnComplete(_) | EventMsg::TurnAborted(_) => Some(true),
            _ => projected,
        })
        .unwrap_or(false);
    let projected_loaded_status = if latest_turn_transition_is_terminal
        && !has_live_in_progress_turn
        && matches!(loaded_status, ThreadStatus::Active { .. })
    {
        ThreadStatus::Idle
    } else {
        loaded_status
    };
    resolve_thread_status(projected_loaded_status, has_live_in_progress_turn)
}
