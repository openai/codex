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

use std::sync::Arc;

use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::Thread;
use codex_app_server_protocol::ThreadHistoryBuilder;
use codex_app_server_protocol::ThreadTokenUsage;
use codex_app_server_protocol::ThreadTokenUsageUpdatedNotification;
use codex_app_server_protocol::Turn;
use codex_app_server_protocol::TurnStatus;
use codex_core::CodexThread;
use codex_protocol::ThreadId;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::TokenUsageInfo;

use crate::outgoing_message::ConnectionId;
use crate::outgoing_message::OutgoingMessageSender;

/// Sends a restored token usage update to the connection that attached to a thread.
///
/// This is lifecycle replay rather than a model event: the rollout already contains
/// the original `TokenCount`, and emitting through `send_event` here would duplicate
/// persisted usage records. Keeping replay connection-scoped also avoids
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
    send_thread_token_usage_snapshot_to_connection(
        outgoing,
        connection_id,
        thread_id,
        thread,
        info,
        token_usage_turn_id,
    )
    .await;
}

/// Sends a token snapshot captured from the same persisted-history cut that supplied its owner.
/// Running-thread resume uses this to avoid pairing a post-cut live value with a pre-cut turn id.
pub(super) async fn send_thread_token_usage_snapshot_to_connection(
    outgoing: &Arc<OutgoingMessageSender>,
    connection_id: ConnectionId,
    thread_id: ThreadId,
    thread: &Thread,
    info: TokenUsageInfo,
    token_usage_turn_id: Option<String>,
) {
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

pub(super) fn latest_token_usage_info_from_rollout_items(
    rollout_items: &[RolloutItem],
) -> Option<TokenUsageInfo> {
    rollout_items.iter().rev().find_map(|item| {
        let RolloutItem::EventMsg(EventMsg::TokenCount(event)) = item else {
            return None;
        };
        event.info.clone()
    })
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
    turns: &[Turn],
) -> Option<String> {
    let mut builder = ThreadHistoryBuilder::new();
    let mut token_usage_turn_owner = None;

    for item in rollout_items {
        if matches!(
            item,
            RolloutItem::EventMsg(EventMsg::TokenCount(event)) if event.info.is_some()
        ) {
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

    let owner = token_usage_turn_owner?;
    if turns.iter().any(|turn| turn.id == owner.id) {
        Some(owner.id)
    } else {
        owner
            .position
            .and_then(|position| turns.get(position))
            .map(|turn| turn.id.clone())
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use codex_app_server_protocol::build_turns_from_rollout_items;
    use codex_protocol::protocol::AgentMessageEvent;
    use codex_protocol::protocol::TokenCountEvent;
    use codex_protocol::protocol::TokenUsage;
    use codex_protocol::protocol::TokenUsageInfo;
    use codex_protocol::protocol::TurnStartedEvent;
    use codex_protocol::protocol::UserMessageEvent;
    use pretty_assertions::assert_eq;

    #[test]
    fn replay_attribution_uses_already_loaded_history() {
        let rollout_items = token_usage_history();
        let turns = build_turns_from_rollout_items(&rollout_items);

        assert_eq!(
            latest_token_usage_turn_id_from_rollout_items(&rollout_items, turns.as_slice()),
            Some(turns[0].id.clone())
        );
    }

    #[test]
    fn replay_attribution_falls_back_to_rebuilt_turn_position() {
        let rollout_items = token_usage_history();
        let mut turns = build_turns_from_rollout_items(&rollout_items);
        turns[0].id = "rebuilt-turn-id".to_string();

        assert_eq!(
            latest_token_usage_turn_id_from_rollout_items(&rollout_items, turns.as_slice()),
            Some("rebuilt-turn-id".to_string())
        );
    }

    #[test]
    fn persisted_cut_captures_token_value_and_owner_without_post_cut_live_state() {
        let usage_b = TokenUsageInfo {
            total_token_usage: TokenUsage {
                total_tokens: 20,
                ..TokenUsage::default()
            },
            last_token_usage: TokenUsage {
                total_tokens: 10,
                ..TokenUsage::default()
            },
            model_context_window: Some(100_000),
        };
        let history_items = vec![
            RolloutItem::EventMsg(EventMsg::TurnStarted(TurnStartedEvent {
                turn_id: "turn-b".to_string(),
                trace_id: None,
                started_at: Some(1),
                model_context_window: Some(100_000),
                collaboration_mode_kind: Default::default(),
            })),
            RolloutItem::EventMsg(EventMsg::TokenCount(TokenCountEvent {
                info: Some(usage_b.clone()),
                rate_limits: None,
            })),
        ];
        let turns = build_turns_from_rollout_items(&history_items);

        assert_eq!(
            latest_token_usage_info_from_rollout_items(&history_items),
            Some(usage_b)
        );
        assert_eq!(
            latest_token_usage_turn_id_from_rollout_items(&history_items, &turns),
            Some("turn-b".to_string())
        );
        // A post-cut turn C is deliberately absent: running resume passes this captured B pair
        // directly to the sender, so live state cannot relabel B or duplicate C.
    }

    #[test]
    fn rate_limit_only_token_event_does_not_relabel_previous_usage_owner() {
        let usage_a = TokenUsageInfo {
            total_token_usage: TokenUsage {
                total_tokens: 10,
                ..TokenUsage::default()
            },
            last_token_usage: TokenUsage {
                total_tokens: 10,
                ..TokenUsage::default()
            },
            model_context_window: Some(100_000),
        };
        let usage_c = TokenUsageInfo {
            total_token_usage: TokenUsage {
                total_tokens: 30,
                ..TokenUsage::default()
            },
            last_token_usage: TokenUsage {
                total_tokens: 20,
                ..TokenUsage::default()
            },
            model_context_window: Some(100_000),
        };
        let mut history = vec![
            RolloutItem::EventMsg(EventMsg::TurnStarted(TurnStartedEvent {
                turn_id: "turn-a".to_string(),
                trace_id: None,
                started_at: Some(1),
                model_context_window: Some(100_000),
                collaboration_mode_kind: Default::default(),
            })),
            RolloutItem::EventMsg(EventMsg::TokenCount(TokenCountEvent {
                info: Some(usage_a.clone()),
                rate_limits: None,
            })),
            RolloutItem::EventMsg(EventMsg::TurnStarted(TurnStartedEvent {
                turn_id: "turn-b".to_string(),
                trace_id: None,
                started_at: Some(2),
                model_context_window: Some(100_000),
                collaboration_mode_kind: Default::default(),
            })),
            RolloutItem::EventMsg(EventMsg::TokenCount(TokenCountEvent {
                info: None,
                rate_limits: None,
            })),
        ];
        let turns = build_turns_from_rollout_items(&history);
        assert_eq!(
            latest_token_usage_info_from_rollout_items(&history),
            Some(usage_a)
        );
        assert_eq!(
            latest_token_usage_turn_id_from_rollout_items(&history, &turns),
            Some("turn-a".to_string())
        );

        history.push(RolloutItem::EventMsg(EventMsg::TurnStarted(
            TurnStartedEvent {
                turn_id: "turn-c".to_string(),
                trace_id: None,
                started_at: Some(3),
                model_context_window: Some(100_000),
                collaboration_mode_kind: Default::default(),
            },
        )));
        history.push(RolloutItem::EventMsg(EventMsg::TokenCount(
            TokenCountEvent {
                info: Some(usage_c.clone()),
                rate_limits: None,
            },
        )));
        let turns = build_turns_from_rollout_items(&history);
        assert_eq!(
            latest_token_usage_info_from_rollout_items(&history),
            Some(usage_c)
        );
        assert_eq!(
            latest_token_usage_turn_id_from_rollout_items(&history, &turns),
            Some("turn-c".to_string())
        );
    }

    fn token_usage_history() -> Vec<RolloutItem> {
        vec![
            RolloutItem::EventMsg(EventMsg::UserMessage(UserMessageEvent {
                client_id: None,
                message: "first turn".to_string(),
                images: None,
                local_images: Vec::new(),
                text_elements: Vec::new(),
                ..Default::default()
            })),
            RolloutItem::EventMsg(EventMsg::AgentMessage(AgentMessageEvent {
                message: "first answer".to_string(),
                phase: None,
                memory_citation: None,
            })),
            RolloutItem::EventMsg(EventMsg::TokenCount(TokenCountEvent {
                info: Some(TokenUsageInfo {
                    total_token_usage: TokenUsage {
                        total_tokens: 10,
                        ..TokenUsage::default()
                    },
                    last_token_usage: TokenUsage {
                        total_tokens: 10,
                        ..TokenUsage::default()
                    },
                    model_context_window: Some(100_000),
                }),
                rate_limits: None,
            })),
            RolloutItem::EventMsg(EventMsg::UserMessage(UserMessageEvent {
                client_id: None,
                message: "second turn".to_string(),
                images: None,
                local_images: Vec::new(),
                text_elements: Vec::new(),
                ..Default::default()
            })),
        ]
    }
}
