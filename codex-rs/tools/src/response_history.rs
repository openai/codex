use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_utils_output_truncation::TruncationPolicy;
use codex_utils_output_truncation::approx_token_count;
use codex_utils_output_truncation::truncate_text;

/// Retains items from the earliest of the last `user_message_count` user
/// messages through the latest user message.
pub fn retain_tail_from_last_n_user_messages(
    items: &mut Vec<ResponseItem>,
    user_message_count: usize,
) {
    if user_message_count == 0 {
        items.clear();
        return;
    }

    let Some(latest_user_idx) = items.iter().rposition(ResponseItem::is_user_message) else {
        items.clear();
        return;
    };
    items.truncate(latest_user_idx + 1);

    let earliest_retained_user_idx = items
        .iter()
        .enumerate()
        .rev()
        .filter(|(_, item)| item.is_user_message())
        .take(user_message_count)
        .last()
        .map(|(idx, _)| idx)
        .unwrap_or(latest_user_idx);
    items.drain(..earliest_retained_user_idx);
}

/// Truncates assistant output text to a shared token budget across items.
pub fn truncate_assistant_output_text_to_token_budget(
    items: &mut Vec<ResponseItem>,
    max_tokens: usize,
) {
    let mut remaining_budget = max_tokens;

    items.retain_mut(|item| {
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
    use codex_protocol::models::ContentItem;
    use codex_protocol::models::ResponseItem;
    use codex_utils_output_truncation::TruncationPolicy;
    use codex_utils_output_truncation::truncate_text;
    use pretty_assertions::assert_eq;

    use super::retain_tail_from_last_n_user_messages;
    use super::truncate_assistant_output_text_to_token_budget;

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
            internal_chat_message_metadata_passthrough: None,
        }
    }

    #[test]
    fn retains_tail_through_latest_user_message() {
        let mut items = vec![
            message("system", "system"),
            message("user", "old user"),
            message("assistant", "old assistant"),
            message("user", "previous user"),
            message("assistant", "previous assistant"),
            message("user", "current user"),
            message("assistant", "later assistant"),
        ];

        retain_tail_from_last_n_user_messages(&mut items, /*user_message_count*/ 2);

        assert_eq!(
            items,
            vec![
                message("user", "previous user"),
                message("assistant", "previous assistant"),
                message("user", "current user"),
            ]
        );
    }

    #[test]
    fn truncates_assistant_output_text_across_items() {
        let long_assistant = "a".repeat(16);
        let mut items = vec![
            message("user", "previous user"),
            message("assistant", &long_assistant),
            message("assistant", "after budget"),
            message("user", "current user"),
        ];

        truncate_assistant_output_text_to_token_budget(&mut items, /*max_tokens*/ 2);

        assert_eq!(
            items,
            vec![
                message("user", "previous user"),
                message(
                    "assistant",
                    &truncate_text(&long_assistant, TruncationPolicy::Tokens(2)),
                ),
                message("user", "current user"),
            ]
        );
    }
}
