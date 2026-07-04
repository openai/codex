//! Delivers pre-cut thread events after the resume response without loss or duplication.

use super::*;

pub(super) fn buffered_event_recipients(
    pre_cut_connection_ids: &[ConnectionId],
    resumed_connection_id: Option<ConnectionId>,
    event_coverage: ResumeEventCoverage,
) -> Vec<ConnectionId> {
    let mut recipients = pre_cut_connection_ids.to_vec();
    if let Some(resumed_connection_id) = resumed_connection_id {
        if event_coverage.represented_in_resume_snapshot
            || !event_coverage.request_live_for_resumed_connection
        {
            recipients.retain(|connection_id| *connection_id != resumed_connection_id);
        } else if !recipients.contains(&resumed_connection_id) {
            recipients.push(resumed_connection_id);
        }
    }
    recipients
}

pub(super) fn buffered_raw_response_recipients(
    pre_cut_connection_ids: &[ConnectionId],
    resumed_connection_id: Option<ConnectionId>,
    routing: BufferedRawResponseRouting,
) -> (Vec<ConnectionId>, Vec<ConnectionId>) {
    let typed_hook_recipients = buffered_event_recipients(
        pre_cut_connection_ids,
        resumed_connection_id,
        routing.event_coverage,
    );
    let mut raw_recipients = if routing.raw_events_enabled {
        pre_cut_connection_ids.to_vec()
    } else {
        Vec::new()
    };
    if let Some(resumed_connection_id) = resumed_connection_id {
        if routing.raw_events_enabled && !routing.resume_payload_mode.is_redacted() {
            if !raw_recipients.contains(&resumed_connection_id) {
                raw_recipients.push(resumed_connection_id);
            }
        } else {
            raw_recipients.retain(|connection_id| *connection_id != resumed_connection_id);
        }
    }
    (typed_hook_recipients, raw_recipients)
}

pub(super) fn buffered_event_delivery_recipients(
    pre_cut_connection_ids: &[ConnectionId],
    resumed_connection_id: Option<ConnectionId>,
    event: &EventMsg,
    event_coverage: ResumeEventCoverage,
) -> (Vec<ConnectionId>, Option<Vec<ConnectionId>>) {
    let item_lifecycle_recipients = buffered_event_recipients(
        pre_cut_connection_ids,
        resumed_connection_id,
        event_coverage,
    );
    let split_item_lifecycle = event_coverage.represented_in_resume_snapshot
        && matches!(
            event,
            EventMsg::GuardianAssessment(_) | EventMsg::DynamicToolCallRequest(_)
        );
    let mut recipients = item_lifecycle_recipients.clone();
    if split_item_lifecycle
        && let Some(resumed_connection_id) = resumed_connection_id
        && (event_coverage.request_live_for_resumed_connection
            || matches!(event, EventMsg::GuardianAssessment(_)))
        && !recipients.contains(&resumed_connection_id)
    {
        // Guardian review notifications and dynamic-tool server requests are not represented by
        // the item snapshot. Deliver those companions to the joiner while routing their
        // already-snapshotted ItemStarted/ItemCompleted lifecycle only to prior subscribers.
        recipients.push(resumed_connection_id);
    }
    (
        recipients,
        split_item_lifecycle.then_some(item_lifecycle_recipients),
    )
}

pub(super) async fn dispatch_replayed_exec_deltas_to_connection(
    replay: ResumeExecDeltaReplay,
    connection_id: ConnectionId,
    conversation_id: ThreadId,
    outgoing: &Arc<OutgoingMessageSender>,
) {
    let outgoing = ThreadScopedOutgoingMessageSender::new(
        Arc::clone(outgoing),
        vec![connection_id],
        conversation_id,
    );
    let thread_id = conversation_id.to_string();
    for buffered in replay.into_events() {
        if buffered.represented_in_resume_snapshot {
            continue;
        }
        let Event {
            id: turn_id,
            msg: EventMsg::ExecCommandOutputDelta(delta),
        } = buffered.event
        else {
            debug_assert!(false, "resume exec replay contains a non-delta event");
            continue;
        };
        let notification = codex_app_server_protocol::item_event_to_server_notification(
            EventMsg::ExecCommandOutputDelta(delta),
            &thread_id,
            &turn_id,
        );
        outgoing.send_server_notification(notification).await;
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn dispatch_buffered_thread_events(
    buffered_events: Vec<BufferedThreadEvent>,
    pre_cut_connection_ids: &[ConnectionId],
    resumed_connection_id: Option<ConnectionId>,
    conversation_id: ThreadId,
    conversation: &Arc<CodexThread>,
    listener_task_context: &ListenerTaskContext,
    thread_state: &Arc<Mutex<ThreadState>>,
    resume_payload_mode: ResumePayloadMode,
) {
    for buffered in buffered_events {
        // Preserve normal listener semantics: every event mutates the live tracker immediately
        // before its bespoke handler observes that state. The resume response used a clone-based
        // final projection, so it never needs to pre-apply the backlog here.
        let raw_events_enabled = track_thread_event(thread_state, &buffered.event).await;
        if let EventMsg::RawResponseItem(raw_response_item_event) = &buffered.event.msg {
            let (typed_hook_recipients, raw_recipients) = buffered_raw_response_recipients(
                pre_cut_connection_ids,
                resumed_connection_id,
                BufferedRawResponseRouting {
                    event_coverage: ResumeEventCoverage {
                        represented_in_resume_snapshot: buffered.represented_in_resume_snapshot,
                        request_live_for_resumed_connection: true,
                    },
                    raw_events_enabled,
                    resume_payload_mode,
                },
            );
            if !typed_hook_recipients.is_empty() {
                let typed_outgoing = ThreadScopedOutgoingMessageSender::new(
                    Arc::clone(&listener_task_context.outgoing),
                    typed_hook_recipients,
                    conversation_id,
                );
                maybe_emit_hook_prompt_item_completed(
                    conversation_id,
                    &buffered.event.id,
                    &raw_response_item_event.item,
                    &typed_outgoing,
                )
                .await;
            }
            if !raw_recipients.is_empty() {
                let raw_outgoing = ThreadScopedOutgoingMessageSender::new(
                    Arc::clone(&listener_task_context.outgoing),
                    raw_recipients,
                    conversation_id,
                );
                maybe_emit_raw_response_item_completed(
                    conversation_id,
                    &buffered.event.id,
                    raw_response_item_event.item.clone(),
                    &raw_outgoing,
                )
                .await;
            }
            continue;
        }
        // Persisted events are already reflected in the resume payload, so suppress their queued
        // notification only for the joiner. Non-persisted stream events cannot be reconstructed
        // and must be replayed after the response instead.
        let (recipients, item_lifecycle_recipients) = buffered_event_delivery_recipients(
            pre_cut_connection_ids,
            resumed_connection_id,
            &buffered.event.msg,
            ResumeEventCoverage {
                represented_in_resume_snapshot: buffered.represented_in_resume_snapshot,
                request_live_for_resumed_connection: buffered.request_live_for_resumed_connection,
            },
        );
        dispatch_thread_event(
            buffered.event,
            conversation_id,
            conversation,
            listener_task_context,
            thread_state,
            recipients,
            item_lifecycle_recipients,
            raw_events_enabled,
        )
        .await;
    }
}
