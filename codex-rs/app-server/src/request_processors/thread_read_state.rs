//! Read state for durable local threads. Receipt changes use the thread's usual subscribers.

use super::*;
use crate::error_code::method_not_found;
use codex_app_server_protocol::ThreadReadState;
use codex_app_server_protocol::ThreadReadStateChangedNotification;
use codex_app_server_protocol::ThreadReadStateOperation;
use codex_app_server_protocol::ThreadReadStateUpdateParams;
use codex_app_server_protocol::ThreadReadStateUpdateResponse;
use codex_app_server_protocol::ThreadUnreadPosition;
use codex_state::ReadStateOperation;
use codex_state::ReadStateUpdate;

// These scopes have product notification policy the local server cannot resolve.
// Do not advertise read state for them until their policy reaches this publisher.
fn eligible(thread: &Thread) -> bool {
    !thread.ephemeral
        && !matches!(
            thread.source,
            codex_app_server_protocol::SessionSource::SubAgent(_)
                | codex_app_server_protocol::SessionSource::Unknown
        )
        && matches!(
            thread.thread_source,
            None | Some(codex_app_server_protocol::ThreadSource::User)
        )
}

fn to_api(state: codex_state::ThreadReadState) -> ThreadReadState {
    ThreadReadState {
        first_unread: state.first_unread_turn.map(|turn_id| {
            if turn_id.is_empty() {
                ThreadUnreadPosition::ThreadStart
            } else {
                ThreadUnreadPosition::Turn { turn_id }
            }
        }),
        revision: state.revision,
    }
}

/// Read receipts only for the local store's list/read results, in one bounded batch.
pub(super) async fn snapshots(
    db: Option<&StateDbHandle>,
    threads: &[Thread],
) -> Option<HashMap<String, ThreadReadState>> {
    let db = db?;
    let ids = threads
        .iter()
        .filter(|t| eligible(t))
        .filter_map(|t| ThreadId::from_string(&t.id).ok())
        .collect::<Vec<_>>();
    match db.thread_read_states(&ids).await {
        Ok(states) => Some(
            states
                .into_iter()
                .map(|(id, state)| (id.to_string(), to_api(state)))
                .collect(),
        ),
        Err(err) => {
            tracing::warn!("thread read state unavailable: {err}");
            None
        }
    }
}

pub(super) async fn notify(
    db: &StateDbHandle,
    manager: &ThreadStateManager,
    outgoing: &Arc<OutgoingMessageSender>,
    id: ThreadId,
) {
    match db.thread_read_states(&[id]).await {
        Ok(mut states) => {
            if let Some(state) = states.remove(&id) {
                let subscribers = manager.subscribed_connection_ids(id).await;
                ThreadScopedOutgoingMessageSender::new(Arc::clone(outgoing), subscribers, id)
                    .send_server_notification(ServerNotification::ThreadReadStateChanged(
                        ThreadReadStateChangedNotification {
                            thread_id: id.to_string(),
                            read_state: to_api(state),
                        },
                    ))
                    .await;
            }
        }
        Err(err) => tracing::warn!("could not project committed read state: {err}"),
    }
}

/// Runs only on newly delivered Core events, never on history decoding or replay.
pub(super) async fn publish(
    event: &codex_protocol::protocol::Event,
    id: ThreadId,
    conversation: &Arc<CodexThread>,
    summary: &crate::thread_state::TurnSummary,
) -> Option<StateDbHandle> {
    let db = conversation.state_db()?;
    let config = conversation.config_snapshot().await;
    if config.ephemeral
        || matches!(
            config.session_source,
            codex_protocol::protocol::SessionSource::SubAgent(_)
                | codex_protocol::protocol::SessionSource::Internal(_)
                | codex_protocol::protocol::SessionSource::Unknown
        )
        || !matches!(
            config.thread_source,
            None | Some(codex_protocol::protocol::ThreadSource::User)
        )
    {
        return None;
    }
    let has_error = summary.last_error.is_some();
    let final_text = match &summary.last_agent_message {
        Some(ThreadItem::AgentMessage { text, .. }) => Some(text.as_str()),
        _ => None,
    };
    let visible = match &event.msg {
        EventMsg::TurnComplete(_) if has_error => true,
        EventMsg::TurnComplete(_) => {
            let text = final_text?;
            if text.contains("<decision>DONT_NOTIFY</decision>") {
                return None;
            }
            // An unfinished goal will resume automatically; it is not a terminal result.
            match db.thread_goals().get_thread_goal(id).await {
                Ok(Some(goal)) if goal.status == codex_state::ThreadGoalStatus::Active => false,
                Ok(_) => !text.trim().is_empty(),
                Err(err) => {
                    tracing::warn!("could not resolve terminal attention policy: {err}");
                    return None;
                }
            }
        }
        EventMsg::TurnAborted(aborted) => {
            aborted.error.is_some() || has_error || final_text.is_some()
        }
        _ => false,
    };
    if visible {
        if let Err(err) = conversation.flush_rollout().await {
            tracing::warn!("cannot publish attention before the terminal result is durable: {err}");
            return None;
        }
        match db.publish_thread_attention(id, &event.id).await {
            Ok(true) => return Some(db),
            Ok(false) => {}
            Err(err) => tracing::warn!("failed to publish terminal attention: {err}"),
        }
    }
    None
}

impl ThreadRequestProcessor {
    pub(crate) async fn thread_read_state_update(
        &self,
        params: ThreadReadStateUpdateParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let id = ThreadId::from_string(&params.thread_id)
            .map_err(|err| invalid_params(format!("invalid thread id: {err}")))?;
        let db = self
            .state_db
            .as_ref()
            .ok_or_else(|| method_not_found("durable thread read state unavailable"))?;
        let thread = self
            .read_thread_view(id, /*include_turns*/ false)
            .await
            .map_err(super::thread_processor::thread_read_view_error)?;
        if !eligible(&thread) {
            return Err(invalid_request("thread read state unavailable"));
        }
        let operation = match params.operation {
            ThreadReadStateOperation::Read => ReadStateOperation::Read,
            ThreadReadStateOperation::Unread => ReadStateOperation::Unread,
        };
        let outcome = db
            .update_thread_read_state(id, &params.expected_revision, operation)
            .await
            .map_err(|err| internal_error(format!("read state was not saved: {err}")))?;
        match outcome {
            ReadStateUpdate::Applied(state) => {
                notify(db, &self.thread_state_manager, &self.outgoing, id).await;
                Ok(Some(
                    ThreadReadStateUpdateResponse {
                        read_state: to_api(state),
                    }
                    .into(),
                ))
            }
            ReadStateUpdate::Conflict(state) => {
                let mut error = invalid_request("read state changed");
                error.data = Some(
                    serde_json::json!({ "reason": "readStateConflict", "readState": to_api(state) }),
                );
                Err(error)
            }
            ReadStateUpdate::Unavailable => Err(invalid_request("thread read state unavailable")),
        }
    }
}
