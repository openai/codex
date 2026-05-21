use codex_api::SearchInput;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::RolloutItem;
use codex_utils_output_truncation::TruncationPolicy;
use codex_utils_output_truncation::approx_token_count;
use codex_utils_output_truncation::truncate_text;

const ASSISTANT_CONTEXT_TOKEN_LIMIT: usize = 1_000;

/// Builds the persisted conversation tail for standalone web search.
///
/// The tail keeps the previous user message, up to 1k tokens of assistant text
/// that followed it, and the current user message.
pub(crate) fn recent_input(items: &[RolloutItem]) -> Option<SearchInput> {
    let messages = recent_messages(items);
    (!messages.is_empty()).then_some(SearchInput::Items(messages))
}

fn recent_messages(items: &[RolloutItem]) -> Vec<ResponseItem> {
    let mut messages = Vec::new();
    for item in items {
        match item {
            RolloutItem::ResponseItem(item) => push_visible_message(&mut messages, item),
            RolloutItem::Compacted(compacted) => {
                if let Some(replacement_history) = &compacted.replacement_history {
                    messages.clear();
                    for item in replacement_history {
                        push_visible_message(&mut messages, item);
                    }
                }
            }
            RolloutItem::EventMsg(EventMsg::ThreadRolledBack(rollback)) => {
                drop_last_user_turns(&mut messages, rollback.num_turns);
            }
            RolloutItem::SessionMeta(_)
            | RolloutItem::TurnContext(_)
            | RolloutItem::EventMsg(_) => {}
        }
    }

    let mut messages = keep_current_and_previous_turn(messages);
    cap_assistant_text(&mut messages);
    messages
}

fn push_visible_message(messages: &mut Vec<ResponseItem>, item: &ResponseItem) {
    if matches!(
        item,
        ResponseItem::Message { role, .. } if role == "user" || role == "assistant"
    ) {
        messages.push(item.clone());
    }
}

fn drop_last_user_turns(messages: &mut Vec<ResponseItem>, count: u32) {
    for _ in 0..count {
        let Some(user_idx) = messages.iter().rposition(is_user_message) else {
            messages.clear();
            return;
        };
        messages.truncate(user_idx);
    }
}

fn is_user_message(item: &ResponseItem) -> bool {
    matches!(item, ResponseItem::Message { role, .. } if role == "user")
}

fn keep_current_and_previous_turn(mut messages: Vec<ResponseItem>) -> Vec<ResponseItem> {
    let Some(current_user_idx) = messages.iter().rposition(is_user_message) else {
        return Vec::new();
    };
    messages.truncate(current_user_idx + 1);
    let previous_user_idx = messages[..current_user_idx]
        .iter()
        .rposition(is_user_message)
        .unwrap_or(current_user_idx);

    messages.drain(..previous_user_idx);
    messages
}

fn cap_assistant_text(messages: &mut Vec<ResponseItem>) {
    let mut remaining_budget = ASSISTANT_CONTEXT_TOKEN_LIMIT;

    messages.retain_mut(|item| {
        let ResponseItem::Message { role, content, .. } = item else {
            return true;
        };
        if role != "assistant" {
            return true;
        }

        content.retain_mut(|content_item| {
            let ContentItem::OutputText { text } = content_item else {
                return true;
            };
            if remaining_budget == 0 {
                return false;
            }

            let token_count = approx_token_count(text);
            if token_count <= remaining_budget {
                remaining_budget = remaining_budget.saturating_sub(token_count);
                return true;
            }

            *text = truncate_text(text, TruncationPolicy::Tokens(remaining_budget));
            remaining_budget = 0;
            true
        });
        !content.is_empty()
    });
}

#[cfg(test)]
mod tests {
    use codex_api::SearchInput;
    use codex_protocol::models::ContentItem;
    use codex_protocol::models::ImageDetail;
    use codex_protocol::models::ResponseItem;
    use codex_protocol::protocol::CompactedItem;
    use codex_protocol::protocol::RolloutItem;
    use codex_utils_output_truncation::TruncationPolicy;
    use codex_utils_output_truncation::truncate_text;
    use pretty_assertions::assert_eq;

    use super::ASSISTANT_CONTEXT_TOKEN_LIMIT;
    use super::recent_input;

    fn message(role: &str, text: &str) -> ResponseItem {
        ResponseItem::Message {
            id: None,
            role: role.to_string(),
            content: vec![if role == "assistant" {
                ContentItem::OutputText {
                    text: text.to_string(),
                }
            } else {
                ContentItem::InputText {
                    text: text.to_string(),
                }
            }],
            phase: None,
        }
    }

    fn rollout_message(role: &str, text: &str) -> RolloutItem {
        RolloutItem::ResponseItem(message(role, text))
    }

    #[test]
    fn keeps_current_user_and_previous_visible_turn() {
        let items = vec![
            rollout_message("system", "system"),
            rollout_message("user", "old user"),
            rollout_message("assistant", "old assistant"),
            rollout_message("user", "previous user"),
            RolloutItem::ResponseItem(ResponseItem::FunctionCall {
                id: None,
                name: "tool".to_string(),
                namespace: None,
                arguments: "{}".to_string(),
                call_id: "call-1".to_string(),
            }),
            rollout_message("assistant", "previous assistant"),
            rollout_message("developer", "developer"),
            rollout_message("user", "current user"),
            rollout_message("assistant", "current commentary"),
        ];

        assert_eq!(
            recent_input(&items),
            Some(SearchInput::Items(vec![
                message("user", "previous user"),
                message("assistant", "previous assistant"),
                message("user", "current user"),
            ]))
        );
    }

    #[test]
    fn uses_compaction_replacement_history() {
        let items = vec![
            rollout_message("user", "stale user"),
            RolloutItem::Compacted(CompactedItem {
                message: "compacted".to_string(),
                replacement_history: Some(vec![
                    message("user", "previous user"),
                    message("assistant", "previous assistant"),
                ]),
            }),
            rollout_message("user", "current user"),
        ];

        assert_eq!(
            recent_input(&items),
            Some(SearchInput::Items(vec![
                message("user", "previous user"),
                message("assistant", "previous assistant"),
                message("user", "current user"),
            ]))
        );
    }

    #[test]
    fn preserves_image_content_from_recent_user_messages() {
        let previous_user = ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![
                ContentItem::InputText {
                    text: "previous user".to_string(),
                },
                ContentItem::InputImage {
                    image_url: "data:image/png;base64,image".to_string(),
                    detail: Some(ImageDetail::High),
                },
            ],
            phase: None,
        };
        let items = vec![
            RolloutItem::ResponseItem(previous_user.clone()),
            rollout_message("assistant", "previous assistant"),
            rollout_message("user", "current user"),
        ];

        assert_eq!(
            recent_input(&items),
            Some(SearchInput::Items(vec![
                previous_user,
                message("assistant", "previous assistant"),
                message("user", "current user"),
            ]))
        );
    }

    #[test]
    fn caps_assistant_text_in_recent_tail() {
        let long_assistant = "a".repeat(4_100);
        let items = vec![
            rollout_message("user", "previous user"),
            rollout_message("assistant", &long_assistant),
            rollout_message("assistant", "after the assistant budget"),
            rollout_message("user", "current user"),
        ];

        assert_eq!(
            recent_input(&items),
            Some(SearchInput::Items(vec![
                message("user", "previous user"),
                message(
                    "assistant",
                    &truncate_text(
                        &long_assistant,
                        TruncationPolicy::Tokens(ASSISTANT_CONTEXT_TOKEN_LIMIT)
                    ),
                ),
                message("user", "current user"),
            ]))
        );
    }
}
