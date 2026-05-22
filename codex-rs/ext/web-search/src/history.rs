use codex_api::SearchInput;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_utils_output_truncation::TruncationPolicy;
use codex_utils_output_truncation::approx_token_count;
use codex_utils_output_truncation::truncate_text;

const ASSISTANT_CONTEXT_TOKEN_LIMIT: usize = 1_000;

/// Builds the conversation tail for standalone web search.
///
/// The tail keeps the previous user text message, up to 1k tokens of assistant
/// text that followed it, and the current user text message.
pub(crate) fn recent_input(items: &[ResponseItem]) -> Option<SearchInput> {
    let mut messages = Vec::new();
    for item in items {
        push_visible_message(&mut messages, item);
    }

    let mut messages = keep_current_and_previous_turn(messages);
    cap_assistant_text(&mut messages);
    (!messages.is_empty()).then_some(SearchInput::Items(messages))
}

fn push_visible_message(messages: &mut Vec<ResponseItem>, item: &ResponseItem) {
    match item {
        ResponseItem::Message { role, .. } if role == "assistant" => messages.push(item.clone()),
        ResponseItem::Message {
            id,
            role,
            content,
            phase,
        } if role == "user" => {
            let content = content
                .iter()
                .filter(|item| matches!(item, ContentItem::InputText { .. }))
                .cloned()
                .collect::<Vec<_>>();
            if !content.is_empty() {
                messages.push(ResponseItem::Message {
                    id: id.clone(),
                    role: role.clone(),
                    content,
                    phase: phase.clone(),
                });
            }
        }
        _ => {}
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
    use codex_protocol::models::ResponseItem;
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

    #[test]
    fn keeps_current_user_and_previous_visible_turn() {
        let items = vec![
            message("system", "system"),
            message("user", "old user"),
            message("assistant", "old assistant"),
            message("user", "previous user"),
            ResponseItem::FunctionCall {
                id: None,
                name: "tool".to_string(),
                namespace: None,
                arguments: "{}".to_string(),
                call_id: "call-1".to_string(),
            },
            message("assistant", "previous assistant"),
            message("developer", "developer"),
            message("user", "current user"),
            message("assistant", "current commentary"),
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
    fn keeps_only_text_from_recent_user_messages() {
        let previous_user = ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![
                ContentItem::InputText {
                    text: "previous user".to_string(),
                },
                ContentItem::InputImage {
                    image_url: "data:image/png;base64,image".to_string(),
                    detail: None,
                },
            ],
            phase: None,
        };
        let items = vec![
            previous_user,
            message("assistant", "previous assistant"),
            message("user", "current user"),
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
    fn caps_assistant_text_in_recent_tail() {
        let long_assistant = "a".repeat(4_100);
        let items = vec![
            message("user", "previous user"),
            message("assistant", &long_assistant),
            message("assistant", "after the assistant budget"),
            message("user", "current user"),
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
