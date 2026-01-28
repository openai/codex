use codex_protocol::items::AgentMessageContent;
use codex_protocol::items::AgentMessageItem;
use codex_protocol::items::ReasoningItem;
use codex_protocol::items::TurnItem;
use codex_protocol::items::UserMessageItem;
use codex_protocol::items::WebSearchItem;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ReasoningItemContent;
use codex_protocol::models::ReasoningItemReasoningSummary;
use codex_protocol::models::ResponseItem;
use codex_protocol::models::WebSearchAction;
use codex_protocol::models::is_image_close_tag_text;
use codex_protocol::models::is_image_open_tag_text;
use codex_protocol::models::is_local_image_close_tag_text;
use codex_protocol::models::is_local_image_open_tag_text;
use codex_protocol::user_input::UserInput;
use tracing::warn;
use uuid::Uuid;

use crate::instructions::SkillInstructions;
use crate::instructions::UserInstructions;
use crate::session_prefix::is_session_prefix;
use crate::user_shell_command::is_user_shell_command_text;
use crate::web_search::web_search_action_detail;

fn parse_user_message(message: &[ContentItem]) -> Option<UserMessageItem> {
    if UserInstructions::is_user_instructions(message)
        || SkillInstructions::is_skill_instructions(message)
    {
        return None;
    }

    let mut content: Vec<UserInput> = Vec::new();

    for (idx, content_item) in message.iter().enumerate() {
        match content_item {
            ContentItem::InputText { text } => {
                if (is_local_image_open_tag_text(text) || is_image_open_tag_text(text))
                    && (matches!(message.get(idx + 1), Some(ContentItem::InputImage { .. })))
                    || (idx > 0
                        && (is_local_image_close_tag_text(text) || is_image_close_tag_text(text))
                        && matches!(message.get(idx - 1), Some(ContentItem::InputImage { .. })))
                {
                    continue;
                }
                if is_session_prefix(text) || is_user_shell_command_text(text) {
                    return None;
                }
                content.push(UserInput::Text {
                    text: text.clone(),
                    // Model input content does not carry UI element ranges.
                    text_elements: Vec::new(),
                });
            }
            ContentItem::InputImage { image_url } => {
                content.push(UserInput::Image {
                    image_url: image_url.clone(),
                });
            }
            ContentItem::OutputText { text } => {
                if is_session_prefix(text) {
                    return None;
                }
                warn!("Output text in user message: {}", text);
            }
        }
    }

    Some(UserMessageItem::new(&content))
}

fn parse_agent_message(id: Option<&String>, message: &[ContentItem]) -> AgentMessageItem {
    let mut content: Vec<AgentMessageContent> = Vec::new();
    for content_item in message.iter() {
        match content_item {
            ContentItem::OutputText { text } => {
                content.push(AgentMessageContent::Text { text: text.clone() });
            }
            _ => {
                warn!(
                    "Unexpected content item in agent message: {:?}",
                    content_item
                );
            }
        }
    }
    let id = id.cloned().unwrap_or_else(|| Uuid::new_v4().to_string());
    AgentMessageItem { id, content }
}

pub fn parse_turn_item(item: &ResponseItem) -> Option<TurnItem> {
    match item {
        ResponseItem::Message {
            role, content, id, ..
        } => match role.as_str() {
            "user" => parse_user_message(content).map(TurnItem::UserMessage),
            "assistant" => Some(TurnItem::AgentMessage(parse_agent_message(
                id.as_ref(),
                content,
            ))),
            "system" => None,
            _ => None,
        },
        ResponseItem::Reasoning {
            id,
            summary,
            content,
            ..
        } => {
            let summary_text = summary
                .iter()
                .map(|entry| match entry {
                    ReasoningItemReasoningSummary::SummaryText { text } => text.clone(),
                })
                .collect();
            let raw_content = content
                .clone()
                .unwrap_or_default()
                .into_iter()
                .map(|entry| match entry {
                    ReasoningItemContent::ReasoningText { text }
                    | ReasoningItemContent::Text { text } => text,
                })
                .collect();
            Some(TurnItem::Reasoning(ReasoningItem {
                id: id.clone(),
                summary_text,
                raw_content,
            }))
        }
        ResponseItem::WebSearchCall { id, action, .. } => {
            let (action, query) = match action {
                Some(action) => (action.clone(), web_search_action_detail(action)),
                None => (WebSearchAction::Other, String::new()),
            };
            Some(TurnItem::WebSearch(WebSearchItem {
                id: id.clone().unwrap_or_default(),
                query,
                action,
            }))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::parse_turn_item;
    use codex_protocol::items::AgentMessageContent;
    use codex_protocol::items::TurnItem;
    use codex_protocol::items::WebSearchItem;
    use codex_protocol::models::ContentItem;
    use codex_protocol::models::ReasoningItemContent;
    use codex_protocol::models::ReasoningItemReasoningSummary;
    use codex_protocol::models::ResponseItem;
    use codex_protocol::models::WebSearchAction;
    use codex_protocol::user_input::UserInput;
    use pretty_assertions::assert_eq;

    #[test]
    fn parses_user_message_with_text_and_two_images() {
        let img1 = "https://example.com/one.png".to_string();
        let img2 = "https://example.com/two.jpg".to_string();

        let item = ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![
                ContentItem::InputText {
                    text: "Hello world".to_string(),
                },
                ContentItem::InputImage {
                    image_url: img1.clone(),
                },
                ContentItem::InputImage {
                    image_url: img2.clone(),
                },
            ],
            end_turn: None,
        };

        let turn_item = parse_turn_item(&item).expect("expected user message turn item");

        match turn_item {
            TurnItem::UserMessage(user) => {
                let expected_content = vec![
                    UserInput::Text {
                        text: "Hello world".to_string(),
                        text_elements: Vec::new(),
                    },
                    UserInput::Image { image_url: img1 },
                    UserInput::Image { image_url: img2 },
                ];
                assert_eq!(user.content, expected_content);
            }
            other => panic!("expected TurnItem::UserMessage, got {other:?}"),
        }
    }

    #[test]
    fn skips_local_image_label_text() {
        let image_url = "data:image/png;base64,abc".to_string();
        let label = codex_protocol::models::local_image_open_tag_text(1);
        let user_text = "Please review this image.".to_string();

        let item = ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![
                ContentItem::InputText { text: label },
                ContentItem::InputImage {
                    image_url: image_url.clone(),
                },
                ContentItem::InputText {
                    text: "</image>".to_string(),
                },
                ContentItem::InputText {
                    text: user_text.clone(),
                },
            ],
            end_turn: None,
        };

        let turn_item = parse_turn_item(&item).expect("expected user message turn item");

        match turn_item {
            TurnItem::UserMessage(user) => {
                let expected_content = vec![
                    UserInput::Image { image_url },
                    UserInput::Text {
                        text: user_text,
                        text_elements: Vec::new(),
                    },
                ];
                assert_eq!(user.content, expected_content);
            }
            other => panic!("expected TurnItem::UserMessage, got {other:?}"),
        }
    }

    #[test]
    fn skips_unnamed_image_label_text() {
        let image_url = "data:image/png;base64,abc".to_string();
        let label = codex_protocol::models::image_open_tag_text();
        let user_text = "Please review this image.".to_string();

        let item = ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![
                ContentItem::InputText { text: label },
                ContentItem::InputImage {
                    image_url: image_url.clone(),
                },
                ContentItem::InputText {
                    text: codex_protocol::models::image_close_tag_text(),
                },
                ContentItem::InputText {
                    text: user_text.clone(),
                },
            ],
            end_turn: None,
        };

        let turn_item = parse_turn_item(&item).expect("expected user message turn item");

        match turn_item {
            TurnItem::UserMessage(user) => {
                let expected_content = vec![
                    UserInput::Image { image_url },
                    UserInput::Text {
                        text: user_text,
                        text_elements: Vec::new(),
                    },
                ];
                assert_eq!(user.content, expected_content);
            }
            other => panic!("expected TurnItem::UserMessage, got {other:?}"),
        }
    }

    #[test]
    fn skips_user_instructions_and_env() {
        let items = vec![
            ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::InputText {
                    text: "<user_instructions>test_text</user_instructions>".to_string(),
                }],
                end_turn: None,
            },
            ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::InputText {
                    text: "<environment_context>test_text</environment_context>".to_string(),
                }],
                end_turn: None,
            },
            ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::InputText {
                    text: "# AGENTS.md instructions for test_directory\n\n<INSTRUCTIONS>\ntest_text\n</INSTRUCTIONS>".to_string(),
                }],
                end_turn: None,
            },
            ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::InputText {
                    text: "<skill>\n<name>demo</name>\n<path>skills/demo/SKILL.md</path>\nbody\n</skill>"
                        .to_string(),
                }],
                end_turn: None,
            },
            ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::InputText {
                    text: "<user_shell_command>echo 42</user_shell_command>".to_string(),
                }],
                end_turn: None,
            },
        ];

        for item in items {
            let turn_item = parse_turn_item(&item);
            assert!(turn_item.is_none(), "expected none, got {turn_item:?}");
        }
    }

    #[test]
    fn parses_agent_message() {
        let item = ResponseItem::Message {
            id: Some("msg-1".to_string()),
            role: "assistant".to_string(),
            content: vec![ContentItem::OutputText {
                text: "Hello from Codex".to_string(),
            }],
            end_turn: None,
        };

        let turn_item = parse_turn_item(&item).expect("expected agent message turn item");

        match turn_item {
            TurnItem::AgentMessage(message) => {
                let Some(AgentMessageContent::Text { text }) = message.content.first() else {
                    panic!("expected agent message text content");
                };
                assert_eq!(text, "Hello from Codex");
            }
            other => panic!("expected TurnItem::AgentMessage, got {other:?}"),
        }
    }

    #[test]
    fn parses_reasoning_summary_and_raw_content() {
        let item = ResponseItem::Reasoning {
            id: "reasoning_1".to_string(),
            summary: vec![
                ReasoningItemReasoningSummary::SummaryText {
                    text: "Step 1".to_string(),
                },
                ReasoningItemReasoningSummary::SummaryText {
                    text: "Step 2".to_string(),
                },
            ],
            content: Some(vec![ReasoningItemContent::ReasoningText {
                text: "raw details".to_string(),
            }]),
            encrypted_content: None,
        };

        let turn_item = parse_turn_item(&item).expect("expected reasoning turn item");

        match turn_item {
            TurnItem::Reasoning(reasoning) => {
                assert_eq!(
                    reasoning.summary_text,
                    vec!["Step 1".to_string(), "Step 2".to_string()]
                );
                assert_eq!(reasoning.raw_content, vec!["raw details".to_string()]);
            }
            other => panic!("expected TurnItem::Reasoning, got {other:?}"),
        }
    }

    #[test]
    fn parses_reasoning_including_raw_content() {
        let item = ResponseItem::Reasoning {
            id: "reasoning_2".to_string(),
            summary: vec![ReasoningItemReasoningSummary::SummaryText {
                text: "Summarized step".to_string(),
            }],
            content: Some(vec![
                ReasoningItemContent::ReasoningText {
                    text: "raw step".to_string(),
                },
                ReasoningItemContent::Text {
                    text: "final thought".to_string(),
                },
            ]),
            encrypted_content: None,
        };

        let turn_item = parse_turn_item(&item).expect("expected reasoning turn item");

        match turn_item {
            TurnItem::Reasoning(reasoning) => {
                assert_eq!(reasoning.summary_text, vec!["Summarized step".to_string()]);
                assert_eq!(
                    reasoning.raw_content,
                    vec!["raw step".to_string(), "final thought".to_string()]
                );
            }
            other => panic!("expected TurnItem::Reasoning, got {other:?}"),
        }
    }

    #[test]
    fn parses_web_search_call() {
        let item = ResponseItem::WebSearchCall {
            id: Some("ws_1".to_string()),
            status: Some("completed".to_string()),
            action: Some(WebSearchAction::Search {
                query: Some("weather".to_string()),
            }),
        };

        let turn_item = parse_turn_item(&item).expect("expected web search turn item");

        match turn_item {
            TurnItem::WebSearch(search) => assert_eq!(
                search,
                WebSearchItem {
                    id: "ws_1".to_string(),
                    query: "weather".to_string(),
                    action: WebSearchAction::Search {
                        query: Some("weather".to_string()),
                    },
                }
            ),
            other => panic!("expected TurnItem::WebSearch, got {other:?}"),
        }
    }

    #[test]
    fn parses_web_search_open_page_call() {
        let item = ResponseItem::WebSearchCall {
            id: Some("ws_open".to_string()),
            status: Some("completed".to_string()),
            action: Some(WebSearchAction::OpenPage {
                url: Some("https://example.com".to_string()),
            }),
        };

        let turn_item = parse_turn_item(&item).expect("expected web search turn item");

        match turn_item {
            TurnItem::WebSearch(search) => assert_eq!(
                search,
                WebSearchItem {
                    id: "ws_open".to_string(),
                    query: "https://example.com".to_string(),
                    action: WebSearchAction::OpenPage {
                        url: Some("https://example.com".to_string()),
                    },
                }
            ),
            other => panic!("expected TurnItem::WebSearch, got {other:?}"),
        }
    }

    #[test]
    fn parses_web_search_find_in_page_call() {
        let item = ResponseItem::WebSearchCall {
            id: Some("ws_find".to_string()),
            status: Some("completed".to_string()),
            action: Some(WebSearchAction::FindInPage {
                url: Some("https://example.com".to_string()),
                pattern: Some("needle".to_string()),
            }),
        };

        let turn_item = parse_turn_item(&item).expect("expected web search turn item");

        match turn_item {
            TurnItem::WebSearch(search) => assert_eq!(
                search,
                WebSearchItem {
                    id: "ws_find".to_string(),
                    query: "'needle' in https://example.com".to_string(),
                    action: WebSearchAction::FindInPage {
                        url: Some("https://example.com".to_string()),
                        pattern: Some("needle".to_string()),
                    },
                }
            ),
            other => panic!("expected TurnItem::WebSearch, got {other:?}"),
        }
    }

    #[test]
    fn parses_partial_web_search_call_without_action_as_other() {
        let item = ResponseItem::WebSearchCall {
            id: Some("ws_partial".to_string()),
            status: Some("in_progress".to_string()),
            action: None,
        };

        let turn_item = parse_turn_item(&item).expect("expected web search turn item");
        match turn_item {
            TurnItem::WebSearch(search) => assert_eq!(
                search,
                WebSearchItem {
                    id: "ws_partial".to_string(),
                    query: String::new(),
                    action: WebSearchAction::Other,
                }
            ),
            other => panic!("expected TurnItem::WebSearch, got {other:?}"),
        }
    }
}
