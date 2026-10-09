//! Read state for durable local threads returned by the thread store.

use super::*;
use codex_app_server_protocol::ThreadReadState;
use codex_app_server_protocol::ThreadUnreadPosition;

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
