use codex_protocol::AgentPath;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum InterAgentDelivery {
    CurrentTurn,
    NextTurn,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct InterAgentInstruction {
    author: AgentPath,
    recipient: AgentPath,
    other_recipients: Vec<AgentPath>,
    content: String,
}

impl InterAgentInstruction {
    pub(crate) fn new(
        author: AgentPath,
        recipient: AgentPath,
        other_recipients: Vec<AgentPath>,
        content: String,
    ) -> Self {
        Self {
            author,
            recipient,
            other_recipients,
            content,
        }
    }

    pub(crate) fn to_response_item(&self) -> ResponseItem {
        ResponseItem::Message {
            id: None,
            role: "assistant".to_string(),
            content: vec![ContentItem::OutputText {
                text: self.as_text(),
            }],
            end_turn: None,
            phase: None,
        }
    }

    pub(crate) fn is_message_content(content: &[ContentItem]) -> bool {
        content.iter().any(|content_item| match content_item {
            ContentItem::InputText { text } | ContentItem::OutputText { text } => {
                Self::is_instruction_text(text)
            }
            _ => false,
        })
    }

    fn as_text(&self) -> String {
        let other_recipients = self
            .other_recipients
            .iter()
            .map(std::string::ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "author: {}\nrecipient: {}\nother_recipients: [{other_recipients}]\nContent: {}",
            self.author, self.recipient, self.content
        )
    }

    fn is_instruction_text(text: &str) -> bool {
        text.starts_with("author: ")
            && text.contains("\nrecipient: ")
            && text.contains("\nother_recipients: [")
            && text.contains("]\nContent: ")
    }
}
