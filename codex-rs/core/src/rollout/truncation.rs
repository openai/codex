//! Helpers for truncating rollouts based on "user turn" boundaries.
//!
//! In core, "user turns" are detected by scanning `ResponseItem::Message` items and
//! interpreting them via `event_mapping::parse_turn_item(...)`.

use crate::event_mapping;
use codex_protocol::items::TurnItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::RolloutItem;

/// Return the indices of user message boundaries in a rollout.
///
/// A user message boundary is a `RolloutItem::ResponseItem(ResponseItem::Message { .. })`
/// whose parsed turn item is `TurnItem::UserMessage`.
///
/// Rollouts can contain `ThreadRolledBack` markers. Those markers indicate that the
/// last N user turns were removed from the effective thread history; we apply them here so
/// indexing uses the post-rollback history rather than the raw stream.
pub(crate) fn user_message_positions_in_rollout(items: &[RolloutItem]) -> Vec<usize> {
    let mut user_positions = Vec::new();
    for (idx, item) in items.iter().enumerate() {
        match item {
            RolloutItem::ResponseItem(item @ ResponseItem::Message { .. })
                if matches!(
                    event_mapping::parse_turn_item(item),
                    Some(TurnItem::UserMessage(_))
                ) =>
            {
                user_positions.push(idx);
            }
            RolloutItem::EventMsg(EventMsg::ThreadRolledBack(rollback)) => {
                let num_turns = usize::try_from(rollback.num_turns).unwrap_or(usize::MAX);
                let new_len = user_positions.len().saturating_sub(num_turns);
                user_positions.truncate(new_len);
            }
            _ => {}
        }
    }
    user_positions
}

/// Return a prefix of `items` obtained by cutting strictly before the nth user message.
///
/// The boundary index is 0-based from the start of `items` (so `n_from_start = 0` returns
/// a prefix that excludes the first user message and everything after it).
///
/// If `n_from_start` is `usize::MAX`, this returns the full rollout (no truncation).
/// If fewer than or equal to `n_from_start` user messages exist, this returns an empty
/// vector (out of range).
pub(crate) fn truncate_rollout_before_nth_user_message_from_start(
    items: &[RolloutItem],
    n_from_start: usize,
) -> Vec<RolloutItem> {
    if n_from_start == usize::MAX {
        return items.to_vec();
    }

    let user_positions = user_message_positions_in_rollout(items);

    // If fewer than or equal to n user messages exist, treat as empty (out of range).
    if user_positions.len() <= n_from_start {
        return Vec::new();
    }

    // Cut strictly before the nth user message (do not keep the nth itself).
    let cut_idx = user_positions[n_from_start];
    items[..cut_idx].to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codex::make_session_and_context;
    use assert_matches::assert_matches;
    use codex_protocol::models::ContentItem;
    use codex_protocol::models::ReasoningItemReasoningSummary;
    use codex_protocol::protocol::ThreadRolledBackEvent;
    use pretty_assertions::assert_eq;

    fn user_msg(text: &str) -> ResponseItem {
        ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::OutputText {
                text: text.to_string(),
            }],
        }
    }

    fn assistant_msg(text: &str) -> ResponseItem {
        ResponseItem::Message {
            id: None,
            role: "assistant".to_string(),
            content: vec![ContentItem::OutputText {
                text: text.to_string(),
            }],
        }
    }

    #[test]
    fn truncates_rollout_from_start_before_nth_user_only() {
        let items = [
            user_msg("u1"),
            assistant_msg("a1"),
            assistant_msg("a2"),
            user_msg("u2"),
            assistant_msg("a3"),
            ResponseItem::Reasoning {
                id: "r1".to_string(),
                summary: vec![ReasoningItemReasoningSummary::SummaryText {
                    text: "s".to_string(),
                }],
                content: None,
                encrypted_content: None,
            },
            ResponseItem::FunctionCall {
                id: None,
                name: "tool".to_string(),
                arguments: "{}".to_string(),
                call_id: "c1".to_string(),
            },
            assistant_msg("a4"),
        ];

        let rollout: Vec<RolloutItem> = items
            .iter()
            .cloned()
            .map(RolloutItem::ResponseItem)
            .collect();

        let truncated = truncate_rollout_before_nth_user_message_from_start(&rollout, 1);
        let expected = vec![
            RolloutItem::ResponseItem(items[0].clone()),
            RolloutItem::ResponseItem(items[1].clone()),
            RolloutItem::ResponseItem(items[2].clone()),
        ];
        assert_eq!(
            serde_json::to_value(&truncated).unwrap(),
            serde_json::to_value(&expected).unwrap()
        );

        let truncated2 = truncate_rollout_before_nth_user_message_from_start(&rollout, 2);
        assert_matches!(truncated2.as_slice(), []);
    }

    #[test]
    fn truncation_max_keeps_full_rollout() {
        let rollout = vec![
            RolloutItem::ResponseItem(user_msg("u1")),
            RolloutItem::ResponseItem(assistant_msg("a1")),
            RolloutItem::ResponseItem(user_msg("u2")),
        ];

        let truncated = truncate_rollout_before_nth_user_message_from_start(&rollout, usize::MAX);

        assert_eq!(
            serde_json::to_value(&truncated).unwrap(),
            serde_json::to_value(&rollout).unwrap()
        );
    }

    #[test]
    fn truncates_rollout_from_start_applies_thread_rollback_markers() {
        let rollout_items = vec![
            RolloutItem::ResponseItem(user_msg("u1")),
            RolloutItem::ResponseItem(assistant_msg("a1")),
            RolloutItem::ResponseItem(user_msg("u2")),
            RolloutItem::ResponseItem(assistant_msg("a2")),
            RolloutItem::EventMsg(EventMsg::ThreadRolledBack(ThreadRolledBackEvent {
                num_turns: 1,
            })),
            RolloutItem::ResponseItem(user_msg("u3")),
            RolloutItem::ResponseItem(assistant_msg("a3")),
            RolloutItem::ResponseItem(user_msg("u4")),
            RolloutItem::ResponseItem(assistant_msg("a4")),
        ];

        // Effective user history after applying rollback(1) is: u1, u3, u4.
        // So n_from_start=2 should cut before u4 (not u3).
        let truncated = truncate_rollout_before_nth_user_message_from_start(&rollout_items, 2);
        let expected = rollout_items[..7].to_vec();
        assert_eq!(
            serde_json::to_value(&truncated).unwrap(),
            serde_json::to_value(&expected).unwrap()
        );
    }

    #[tokio::test]
    async fn ignores_session_prefix_messages_when_truncating_rollout_from_start() {
        let (session, turn_context) = make_session_and_context().await;
        let mut items = session.build_initial_context(&turn_context);
        items.push(user_msg("feature request"));
        items.push(assistant_msg("ack"));
        items.push(user_msg("second question"));
        items.push(assistant_msg("answer"));

        let rollout_items: Vec<RolloutItem> = items
            .iter()
            .cloned()
            .map(RolloutItem::ResponseItem)
            .collect();

        let truncated = truncate_rollout_before_nth_user_message_from_start(&rollout_items, 1);
        let expected: Vec<RolloutItem> = vec![
            RolloutItem::ResponseItem(items[0].clone()),
            RolloutItem::ResponseItem(items[1].clone()),
            RolloutItem::ResponseItem(items[2].clone()),
            RolloutItem::ResponseItem(items[3].clone()),
        ];

        assert_eq!(
            serde_json::to_value(&truncated).unwrap(),
            serde_json::to_value(&expected).unwrap()
        );
    }
}
