use codex_api::SearchInput;
use codex_core::parse_turn_item;
use codex_protocol::items::TurnItem;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_tools::retain_tail_from_last_n_user_messages;
use codex_tools::truncate_assistant_output_text_to_token_budget;

const ASSISTANT_CONTEXT_TOKEN_LIMIT: usize = 1_000;
const ASSISTANT_ROLE: &str = "assistant";
const USER_ROLE: &str = "user";

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
        ResponseItem::Message { role, .. } if role == ASSISTANT_ROLE => {
            messages.push(item.clone());
        }
        ResponseItem::Message {
            id,
            role,
            content,
            phase,
        } if role == USER_ROLE
            && matches!(parse_turn_item(item), Some(TurnItem::UserMessage(_))) =>
        {
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

    use super::ASSISTANT_ROLE;
    use super::USER_ROLE;
    use super::recent_input;

    fn message(role: &str, text: &str) -> ResponseItem {
        ResponseItem::Message {
            id: None,
            role: role.to_string(),
            content: vec![if role == ASSISTANT_ROLE {
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
            message(USER_ROLE, "old user"),
            message(ASSISTANT_ROLE, "old assistant"),
            message(USER_ROLE, "previous user"),
            ResponseItem::FunctionCall {
                id: None,
                name: "tool".to_string(),
                namespace: None,
                arguments: "{}".to_string(),
                call_id: "call-1".to_string(),
            },
            message(ASSISTANT_ROLE, "previous assistant"),
            message("developer", "developer"),
            message(USER_ROLE, "current user"),
            message(ASSISTANT_ROLE, "current commentary"),
        ];

        assert_eq!(
            recent_input(&items),
            Some(SearchInput::Items(vec![
                message(USER_ROLE, "previous user"),
                message(ASSISTANT_ROLE, "previous assistant"),
                message(USER_ROLE, "current user"),
            ]))
        );
    }

    #[test]
    fn keeps_only_text_from_recent_user_messages() {
        let previous_user = ResponseItem::Message {
            id: None,
            role: USER_ROLE.to_string(),
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
            message(ASSISTANT_ROLE, "previous assistant"),
            message(USER_ROLE, "current user"),
        ];

        assert_eq!(
            recent_input(&items),
            Some(SearchInput::Items(vec![
                message(USER_ROLE, "previous user"),
                message(ASSISTANT_ROLE, "previous assistant"),
                message(USER_ROLE, "current user"),
            ]))
        );
    }

    #[test]
    fn ignores_contextual_user_messages_when_selecting_recent_turns() {
        let items = vec![
            message(USER_ROLE, "previous user"),
            message(ASSISTANT_ROLE, "previous assistant"),
            message(
                USER_ROLE,
                "<environment_context>\n<cwd>/tmp</cwd>\n</environment_context>",
            ),
            message(USER_ROLE, "current user"),
        ];

        assert_eq!(
            recent_input(&items),
            Some(SearchInput::Items(vec![
                message(USER_ROLE, "previous user"),
                message(ASSISTANT_ROLE, "previous assistant"),
                message(USER_ROLE, "current user"),
            ]))
        );
    }
}
