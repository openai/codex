//! Helpers for truncating conversation history based on "user turn" boundaries.
//!
//! In core, "user turns" are detected by scanning `ResponseItem::Message` items and
//! interpreting them via `event_mapping::parse_turn_item(...)`.
//!
//! These helpers are intentionally pure (no I/O, no spawning) so they can be reused by:
//! - `ConversationManager` (forking via rollout truncation)
//! - `Session` (live rollback via in-memory `ResponseItem` truncation)

use crate::event_mapping;
use codex_protocol::items::TurnItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::RolloutItem;

/// Return the indices of user message boundaries in a rollout stream.
///
/// A user message boundary is a `RolloutItem::ResponseItem(ResponseItem::Message { .. })`
/// whose parsed turn item is `TurnItem::UserMessage`.
pub(crate) fn user_message_positions_in_rollout(items: &[RolloutItem]) -> Vec<usize> {
    let mut user_positions = Vec::new();
    for (idx, item) in items.iter().enumerate() {
        if let RolloutItem::ResponseItem(item @ ResponseItem::Message { .. }) = item
            && matches!(
                event_mapping::parse_turn_item(item),
                Some(TurnItem::UserMessage(_))
            )
        {
            user_positions.push(idx);
        }
    }
    user_positions
}

/// Return a prefix of `items` obtained by cutting strictly before the nth user message.
///
/// The boundary index is 0-based from the start of `items` (so `n_from_start = 0` returns
/// a prefix that excludes the first user message and everything after it).
///
/// If fewer than or equal to `n_from_start` user messages exist, this returns an empty
/// vector (out of range).
pub(crate) fn truncate_rollout_before_nth_user_message_from_start(
    items: &[RolloutItem],
    n_from_start: usize,
) -> Vec<RolloutItem> {
    // Work directly on rollout items, and cut the vector at the nth user message input.
    let user_positions = user_message_positions_in_rollout(items);
    if user_positions.len() <= n_from_start {
        return Vec::new();
    }

    // Cut strictly before the nth user message (do not keep the nth itself).
    let cut_idx = user_positions[n_from_start];
    items[..cut_idx].to_vec()
}

/// Return the indices of user message boundaries in an in-memory transcript.
///
/// A user message boundary is a `ResponseItem::Message { .. }` whose parsed turn item is
/// `TurnItem::UserMessage`.
pub(crate) fn user_message_positions_in_response_items(items: &[ResponseItem]) -> Vec<usize> {
    let mut user_positions = Vec::new();
    for (idx, item) in items.iter().enumerate() {
        if let ResponseItem::Message { .. } = item
            && matches!(
                event_mapping::parse_turn_item(item),
                Some(TurnItem::UserMessage(_))
            )
        {
            user_positions.push(idx);
        }
    }
    user_positions
}

/// Return a prefix of `items` obtained by dropping the last `num_turns` user turns.
///
/// This is intended to implement "thread rollback" semantics:
/// - `num_turns == 0` is a no-op
/// - if there are no user turns, this is a no-op
/// - if `num_turns` exceeds the number of user turns, all user turns are dropped while
///   preserving any "session prefix" items that occurred before the first user message.
pub(crate) fn drop_last_n_user_turns_from_response_items(
    items: &[ResponseItem],
    num_turns: u32,
) -> Vec<ResponseItem> {
    if num_turns == 0 {
        return items.to_vec();
    }

    let user_positions = user_message_positions_in_response_items(items);
    let Some(&first_user_idx) = user_positions.first() else {
        return items.to_vec();
    };

    let n_from_end = usize::try_from(num_turns).unwrap_or(usize::MAX);
    let cut_idx = if n_from_end >= user_positions.len() {
        first_user_idx
    } else {
        user_positions[user_positions.len() - n_from_end]
    };

    items[..cut_idx].to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codex::make_session_and_context;
    use assert_matches::assert_matches;
    use codex_protocol::models::ContentItem;
    use codex_protocol::models::ReasoningItemReasoningSummary;
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
        ];

        assert_eq!(
            serde_json::to_value(&truncated).unwrap(),
            serde_json::to_value(&expected).unwrap()
        );
    }

    #[test]
    fn drops_last_n_user_turns_from_response_items_preserves_prefix() {
        let items = vec![
            assistant_msg("session prefix item"),
            user_msg("u1"),
            assistant_msg("a1"),
            user_msg("u2"),
            assistant_msg("a2"),
        ];

        let got = drop_last_n_user_turns_from_response_items(&items, 1);
        let expected = vec![
            assistant_msg("session prefix item"),
            user_msg("u1"),
            assistant_msg("a1"),
        ];
        assert_eq!(
            serde_json::to_value(&got).unwrap(),
            serde_json::to_value(&expected).unwrap()
        );

        let got2 = drop_last_n_user_turns_from_response_items(&items, 99);
        let expected2 = vec![assistant_msg("session prefix item")];
        assert_eq!(
            serde_json::to_value(&got2).unwrap(),
            serde_json::to_value(&expected2).unwrap()
        );
    }
}
