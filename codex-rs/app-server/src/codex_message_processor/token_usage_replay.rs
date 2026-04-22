//! Replays persisted token usage snapshots when a client attaches to an existing thread.
//!
//! The message processor decides when replay is allowed and preserves JSON-RPC response
//! ordering. This module owns notification construction and the attribution rules that
//! map the latest persisted `TokenCount` back to a v2 turn id.
//!
//! Rollout histories can contain explicit turn ids or generated turn ids. When explicit
//! ids do not match the rebuilt thread, replay falls back to the active turn position at
//! the time the `TokenCount` was persisted so the notification still targets the
//! corresponding rebuilt turn.

use std::path::Path;
use std::sync::Arc;

use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::Thread;
use codex_app_server_protocol::ThreadHistoryBuilder;
use codex_app_server_protocol::ThreadTokenUsage;
use codex_app_server_protocol::ThreadTokenUsageUpdatedNotification;
use codex_app_server_protocol::TurnStatus;
use codex_core::CodexThread;
use codex_protocol::ThreadId;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::RolloutItem;

use crate::codex_message_processor::read_rollout_items_from_rollout;
use crate::outgoing_message::ConnectionId;
use crate::outgoing_message::OutgoingMessageSender;

/// Sends a restored token usage update to the connection that attached to a thread.
///
/// This is lifecycle replay rather than a model event: the rollout already contains
/// the original `TokenCount`, and emitting through `send_event` here would duplicate
/// persisted usage records. Keeping this helper connection-scoped also avoids
/// surprising other subscribers with a historical usage update while they may be
/// rendering live turn events.
pub(super) async fn send_thread_token_usage_update_to_connection(
    outgoing: &Arc<OutgoingMessageSender>,
    connection_id: ConnectionId,
    thread_id: ThreadId,
    thread: &Thread,
    conversation: &CodexThread,
    token_usage_turn_id: Option<String>,
) {
    let Some(info) = conversation.token_usage_info().await else {
        return;
    };
    let notification = ThreadTokenUsageUpdatedNotification {
        thread_id: thread_id.to_string(),
        turn_id: token_usage_turn_id.unwrap_or_else(|| latest_token_usage_turn_id(thread)),
        token_usage: ThreadTokenUsage::from(info),
    };
    outgoing
        .send_server_notification_to_connections(
            &[connection_id],
            ServerNotification::ThreadTokenUsageUpdated(notification),
        )
        .await;
}

pub(super) async fn latest_token_usage_turn_id_for_thread_path(thread: &Thread) -> Option<String> {
    let rollout_path = thread.path.as_deref()?;
    latest_token_usage_turn_id_from_rollout_path(rollout_path).await
}

pub(super) async fn latest_token_usage_turn_id_from_rollout_path(
    rollout_path: &Path,
) -> Option<String> {
    let rollout_items = read_rollout_items_from_rollout(rollout_path).await.ok()?;
    latest_token_usage_turn_id_from_rollout_items(&rollout_items)
}

/// Identifies the turn that was active when a `TokenCount` record appeared.
///
/// The id is preferred when it still appears in the rebuilt thread. The position is a
/// fallback for histories whose implicit turn ids are regenerated during reconstruction.
struct TokenUsageTurnOwner {
    id: String,
    position: Option<usize>,
}

pub(super) fn latest_token_usage_turn_id_from_rollout_items(
    rollout_items: &[RolloutItem],
) -> Option<String> {
    let owner = latest_token_usage_turn_owner_from_rollout_items(rollout_items)?;
    let rebuilt_turns = super::build_turns_from_rollout_items(rollout_items);
    if rebuilt_turns.iter().any(|turn| turn.id == owner.id) {
        return Some(owner.id);
    }
    owner
        .position
        .and_then(|position| rebuilt_turns.get(position))
        .map(|turn| turn.id.clone())
}

fn latest_token_usage_turn_owner_from_rollout_items(
    rollout_items: &[RolloutItem],
) -> Option<TokenUsageTurnOwner> {
    let mut builder = ThreadHistoryBuilder::new();
    let mut token_usage_turn_owner = None;

    for item in rollout_items {
        if matches!(item, RolloutItem::EventMsg(EventMsg::TokenCount(_))) {
            token_usage_turn_owner =
                builder
                    .active_turn_snapshot()
                    .map(|turn| TokenUsageTurnOwner {
                        id: turn.id,
                        position: builder.active_turn_position(),
                    });
        }
        builder.handle_rollout_item(item);
    }

    token_usage_turn_owner
}

/// Chooses a fallback turn id that should own a replayed token usage update.
///
/// Normal replay derives the owner from the rollout position of the latest
/// `TokenCount` event. This fallback only preserves a stable wire shape for
/// unusual histories where that rollout information cannot be read.
fn latest_token_usage_turn_id(thread: &Thread) -> String {
    thread
        .turns
        .iter()
        .rev()
        .find(|turn| matches!(turn.status, TurnStatus::Completed | TurnStatus::Failed))
        .or_else(|| thread.turns.last())
        .map(|turn| turn.id.clone())
        .unwrap_or_default()
}
