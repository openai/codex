use codex_api::SearchInput;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_tools::retain_tail_from_last_n_user_messages;
use codex_tools::truncate_assistant_output_text_to_token_budget;

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

    retain_tail_from_last_n_user_messages(&mut messages, /*user_message_count*/ 2);
    truncate_assistant_output_text_to_token_budget(&mut messages, ASSISTANT_CONTEXT_TOKEN_LIMIT);
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

#[cfg(test)]
mod tests {
    use codex_api::SearchInput;
    use codex_protocol::models::ContentItem;
    use codex_protocol::models::ResponseItem;
    use pretty_assertions::assert_eq;

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
}
