use crate::compact;
use crate::compact::collect_user_messages;
use crate::context_manager::is_user_turn_boundary;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::RolloutItem;

/// Replays rollout items into effective response history.
///
/// This applies compaction and rollback markers so callers can reconstruct
/// the post-replay thread state instead of the raw persisted stream.
pub fn replay_rollout_response_items(rollout_items: &[RolloutItem]) -> Vec<ResponseItem> {
    replay_rollout_response_items_with_initial_context(rollout_items, infer_initial_context)
}

/// Replays rollout items into effective response history, with a custom initial-context provider
/// used when rebuilding compaction entries that do not have replacement history.
pub fn replay_rollout_response_items_with_initial_context<F>(
    rollout_items: &[RolloutItem],
    mut initial_context: F,
) -> Vec<ResponseItem>
where
    F: FnMut(&[ResponseItem]) -> Vec<ResponseItem>,
{
    let mut history = Vec::new();
    for item in rollout_items {
        match item {
            RolloutItem::ResponseItem(response_item) => {
                history.push(response_item.clone());
            }
            RolloutItem::Compacted(compacted) => {
                if let Some(replacement) = &compacted.replacement_history {
                    history = replacement.clone();
                } else {
                    let initial_context = initial_context(&history);
                    let user_messages = collect_user_messages(&history);
                    history = compact::build_compacted_history(
                        initial_context,
                        &user_messages,
                        &compacted.message,
                    );
                }
            }
            RolloutItem::EventMsg(EventMsg::ThreadRolledBack(rollback)) => {
                drop_last_n_user_turns(&mut history, rollback.num_turns);
            }
            _ => {}
        }
    }
    history
}

fn infer_initial_context(history: &[ResponseItem]) -> Vec<ResponseItem> {
    match history.iter().position(is_user_turn_boundary) {
        Some(first_user_idx) => history[..first_user_idx].to_vec(),
        None => history.to_vec(),
    }
}

fn drop_last_n_user_turns(history: &mut Vec<ResponseItem>, num_turns: u32) {
    if num_turns == 0 {
        return;
    }

    let user_positions = user_message_positions(history);
    let Some(&first_user_idx) = user_positions.first() else {
        return;
    };

    let n_from_end = usize::try_from(num_turns).unwrap_or(usize::MAX);
    let cut_idx = if n_from_end >= user_positions.len() {
        first_user_idx
    } else {
        user_positions[user_positions.len() - n_from_end]
    };
    history.truncate(cut_idx);
}

fn user_message_positions(items: &[ResponseItem]) -> Vec<usize> {
    let mut positions = Vec::new();
    for (idx, item) in items.iter().enumerate() {
        if is_user_turn_boundary(item) {
            positions.push(idx);
        }
    }
    positions
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_protocol::models::ContentItem;
    use codex_protocol::protocol::CompactedItem;
    use codex_protocol::protocol::ThreadRolledBackEvent;
    use pretty_assertions::assert_eq;

    fn user_msg(text: &str) -> ResponseItem {
        ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: text.to_string(),
            }],
            end_turn: None,
            phase: None,
        }
    }

    fn assistant_msg(text: &str) -> ResponseItem {
        ResponseItem::Message {
            id: None,
            role: "assistant".to_string(),
            content: vec![ContentItem::OutputText {
                text: text.to_string(),
            }],
            end_turn: None,
            phase: None,
        }
    }

    fn system_msg(text: &str) -> ResponseItem {
        ResponseItem::Message {
            id: None,
            role: "system".to_string(),
            content: vec![ContentItem::OutputText {
                text: text.to_string(),
            }],
            end_turn: None,
            phase: None,
        }
    }

    #[test]
    fn compaction_replay_uses_replacement_history_when_present() {
        let replacement = vec![user_msg("replacement summary")];
        let rollout = vec![
            RolloutItem::ResponseItem(user_msg("before")),
            RolloutItem::ResponseItem(assistant_msg("assistant before")),
            RolloutItem::Compacted(CompactedItem {
                message: "summary".to_string(),
                replacement_history: Some(replacement.clone()),
            }),
            RolloutItem::ResponseItem(assistant_msg("after")),
        ];

        let replayed = replay_rollout_response_items(&rollout);
        let expected = vec![replacement[0].clone(), assistant_msg("after")];
        assert_eq!(replayed, expected);
    }

    #[test]
    fn compaction_replay_rebuilds_when_replacement_history_absent() {
        let rollout = vec![
            RolloutItem::ResponseItem(system_msg("prefix")),
            RolloutItem::ResponseItem(user_msg("first user")),
            RolloutItem::ResponseItem(assistant_msg("assistant reply")),
            RolloutItem::Compacted(CompactedItem {
                message: "summary".to_string(),
                replacement_history: None,
            }),
        ];

        let replayed = replay_rollout_response_items(&rollout);
        let expected = vec![
            system_msg("prefix"),
            user_msg("first user"),
            user_msg("summary"),
        ];
        assert_eq!(replayed, expected);
    }

    #[test]
    fn rollback_marker_applies_after_compaction_replay() {
        let rollout = vec![
            RolloutItem::ResponseItem(user_msg("before")),
            RolloutItem::ResponseItem(assistant_msg("assistant before")),
            RolloutItem::Compacted(CompactedItem {
                message: "summary".to_string(),
                replacement_history: Some(vec![user_msg("summary")]),
            }),
            RolloutItem::ResponseItem(user_msg("latest")),
            RolloutItem::ResponseItem(assistant_msg("assistant latest")),
            RolloutItem::EventMsg(EventMsg::ThreadRolledBack(ThreadRolledBackEvent {
                num_turns: 1,
            })),
        ];

        let replayed = replay_rollout_response_items(&rollout);
        assert_eq!(replayed, vec![user_msg("summary")]);
    }

    #[test]
    fn multiple_rollback_markers_apply_in_sequence() {
        let rollout = vec![
            RolloutItem::ResponseItem(system_msg("prefix")),
            RolloutItem::ResponseItem(user_msg("u1")),
            RolloutItem::ResponseItem(assistant_msg("a1")),
            RolloutItem::ResponseItem(user_msg("u2")),
            RolloutItem::ResponseItem(assistant_msg("a2")),
            RolloutItem::EventMsg(EventMsg::ThreadRolledBack(ThreadRolledBackEvent {
                num_turns: 1,
            })),
            RolloutItem::EventMsg(EventMsg::ThreadRolledBack(ThreadRolledBackEvent {
                num_turns: 1,
            })),
            RolloutItem::EventMsg(EventMsg::ThreadRolledBack(ThreadRolledBackEvent {
                num_turns: 1,
            })),
        ];

        let replayed = replay_rollout_response_items(&rollout);
        assert_eq!(replayed, vec![system_msg("prefix")]);
    }
}
