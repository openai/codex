use std::io;
use std::io::ErrorKind;
use std::path::Path;

use codex_instructions::ContextualUserFragmentDefinition;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::RolloutItem;
use codex_utils_output_truncation::TruncationPolicy;

use crate::codex::rollout_reconstruction::reconstructed_history_from_rollout_items;
use crate::compact::content_items_to_text;
use crate::event_mapping::is_contextual_user_message_content;
use crate::rollout::RolloutRecorder;

pub(crate) const REMEMBERED_CONTEXT_OPEN_TAG: &str = "<remembered_context>";
const REMEMBERED_CONTEXT_CLOSE_TAG: &str = "</remembered_context>";

pub(crate) const REMEMBERED_CONTEXT_FRAGMENT: ContextualUserFragmentDefinition =
    ContextualUserFragmentDefinition::new(
        REMEMBERED_CONTEXT_OPEN_TAG,
        REMEMBERED_CONTEXT_CLOSE_TAG,
    );

const ROLLOUT_RECONSTRUCTION_TRUNCATION_BYTES: usize = 10 * 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RememberedConversation {
    pub messages: Vec<RememberedConversationMessage>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RememberedConversationMessage {
    User { text: String },
    Assistant { text: String },
}

pub async fn load_remembered_conversation_from_rollout(
    rollout_path: &Path,
) -> io::Result<RememberedConversation> {
    let (rollout_items, _thread_id, _parse_errors) =
        RolloutRecorder::load_rollout_items(rollout_path).await?;
    remembered_conversation_from_rollout_items(&rollout_items)
}

fn remembered_conversation_from_rollout_items(
    rollout_items: &[RolloutItem],
) -> io::Result<RememberedConversation> {
    let history = reconstructed_history_from_rollout_items(
        rollout_items,
        TruncationPolicy::Bytes(ROLLOUT_RECONSTRUCTION_TRUNCATION_BYTES),
    );
    let messages = remembered_messages_from_response_items(&history);
    if messages.is_empty() {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            "source thread has no visible user or assistant messages",
        ));
    }

    Ok(RememberedConversation { messages })
}

fn remembered_messages_from_response_items(
    history: &[ResponseItem],
) -> Vec<RememberedConversationMessage> {
    history
        .iter()
        .filter_map(|item| match item {
            ResponseItem::Message { role, content, .. } if role == "user" => {
                if is_contextual_user_message_content(content) {
                    return None;
                }
                content_items_to_text(content).map(|text| RememberedConversationMessage::User {
                    text: text.trim().to_string(),
                })
            }
            ResponseItem::Message { role, content, .. } if role == "assistant" => {
                content_items_to_text(content).map(|text| {
                    RememberedConversationMessage::Assistant {
                        text: text.trim().to_string(),
                    }
                })
            }
            ResponseItem::Message { .. }
            | ResponseItem::Reasoning { .. }
            | ResponseItem::LocalShellCall { .. }
            | ResponseItem::FunctionCall { .. }
            | ResponseItem::ToolSearchCall { .. }
            | ResponseItem::FunctionCallOutput { .. }
            | ResponseItem::CustomToolCall { .. }
            | ResponseItem::CustomToolCallOutput { .. }
            | ResponseItem::ToolSearchOutput { .. }
            | ResponseItem::WebSearchCall { .. }
            | ResponseItem::ImageGenerationCall { .. }
            | ResponseItem::GhostSnapshot { .. }
            | ResponseItem::Compaction { .. }
            | ResponseItem::Other => None,
        })
        .filter(|message| !message.text().is_empty())
        .collect()
}

pub fn remembered_context_response_item(context: String) -> ResponseItem {
    ResponseItem::Message {
        id: None,
        role: "user".to_string(),
        content: vec![ContentItem::InputText {
            text: format!(
                "{REMEMBERED_CONTEXT_OPEN_TAG}\n{context}\n{REMEMBERED_CONTEXT_CLOSE_TAG}"
            ),
        }],
        end_turn: None,
        phase: None,
    }
}

impl RememberedConversationMessage {
    pub fn text(&self) -> &str {
        match self {
            Self::User { text } | Self::Assistant { text } => text,
        }
    }
}

#[cfg(test)]
mod tests {
    use codex_protocol::config_types::ModeKind;
    use codex_protocol::protocol::CompactedItem;
    use codex_protocol::protocol::EventMsg;
    use codex_protocol::protocol::ThreadRolledBackEvent;
    use codex_protocol::protocol::TurnCompleteEvent;
    use codex_protocol::protocol::TurnStartedEvent;
    use codex_protocol::protocol::UserMessageEvent;

    use super::*;
    use pretty_assertions::assert_eq;

    fn user_message(text: &str) -> ResponseItem {
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

    fn assistant_message(text: &str) -> ResponseItem {
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

    fn persisted_turn(turn_id: &str, user: &str, assistant: &str) -> Vec<RolloutItem> {
        vec![
            RolloutItem::EventMsg(EventMsg::TurnStarted(TurnStartedEvent {
                turn_id: turn_id.to_string(),
                started_at: None,
                model_context_window: Some(128_000),
                collaboration_mode_kind: ModeKind::Default,
            })),
            RolloutItem::EventMsg(EventMsg::UserMessage(UserMessageEvent {
                message: user.to_string(),
                images: None,
                local_images: Vec::new(),
                text_elements: Vec::new(),
            })),
            RolloutItem::ResponseItem(user_message(user)),
            RolloutItem::ResponseItem(assistant_message(assistant)),
            RolloutItem::EventMsg(EventMsg::TurnComplete(TurnCompleteEvent {
                turn_id: turn_id.to_string(),
                last_agent_message: Some(assistant.to_string()),
                completed_at: None,
                duration_ms: None,
            })),
        ]
    }

    #[test]
    fn extracts_visible_user_and_assistant_messages() {
        let hidden_context = remembered_context_response_item("old hidden note".to_string());
        let items = vec![
            user_message("first request"),
            hidden_context,
            assistant_message("first answer"),
        ];

        assert_eq!(
            remembered_messages_from_response_items(&items),
            vec![
                RememberedConversationMessage::User {
                    text: "first request".to_string(),
                },
                RememberedConversationMessage::Assistant {
                    text: "first answer".to_string(),
                },
            ]
        );
    }

    #[test]
    fn extracts_surviving_compacted_replacement_history() {
        let replacement_history = vec![user_message("surviving compacted request")];
        let items = vec![
            RolloutItem::ResponseItem(user_message("rolled-away request")),
            RolloutItem::Compacted(CompactedItem {
                message: "summary".to_string(),
                replacement_history: Some(replacement_history),
            }),
            RolloutItem::ResponseItem(assistant_message("new answer")),
        ];

        let conversation = remembered_conversation_from_rollout_items(&items).unwrap();

        assert_eq!(
            conversation,
            RememberedConversation {
                messages: vec![
                    RememberedConversationMessage::User {
                        text: "surviving compacted request".to_string(),
                    },
                    RememberedConversationMessage::Assistant {
                        text: "new answer".to_string(),
                    },
                ],
            }
        );
    }

    #[test]
    fn excludes_rolled_back_tail() {
        let mut items = persisted_turn("turn-1", "kept request", "kept answer");
        items.extend(persisted_turn(
            "turn-2",
            "dropped request",
            "dropped answer",
        ));
        items.push(RolloutItem::EventMsg(EventMsg::ThreadRolledBack(
            ThreadRolledBackEvent { num_turns: 1 },
        )));

        let conversation = remembered_conversation_from_rollout_items(&items).unwrap();

        assert_eq!(
            conversation,
            RememberedConversation {
                messages: vec![
                    RememberedConversationMessage::User {
                        text: "kept request".to_string(),
                    },
                    RememberedConversationMessage::Assistant {
                        text: "kept answer".to_string(),
                    },
                ],
            }
        );
    }
}
