use serde::Deserialize;
use serde::Serialize;

use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::USER_INSTRUCTIONS_CLOSE_TAG;
use codex_protocol::protocol::USER_INSTRUCTIONS_OPEN_TAG;

/// Wraps user instructions in a tag so the model can classify them easily.

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename = "user_instructions", rename_all = "snake_case")]
pub(crate) struct UserInstructions {
    text: String,
}

impl UserInstructions {
    pub fn new<T: Into<String>>(text: T) -> Self {
        Self { text: text.into() }
    }

    /// Serializes the user instructions to an XML-like tagged block that starts
    /// with <user_instructions> so clients can classify it.
    pub fn serialize_to_xml(self) -> String {
        format!(
            "{USER_INSTRUCTIONS_OPEN_TAG}\n\n{}\n\n{USER_INSTRUCTIONS_CLOSE_TAG}",
            self.text
        )
    }
}

impl From<UserInstructions> for ResponseItem {
    fn from(ui: UserInstructions) -> Self {
        ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: ui.serialize_to_xml(),
            }],
        }
    }
}
