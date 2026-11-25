//! Present delivered replies as tool evidence without changing the reviewed agent's history.

use codex_protocol::AgentPath;
use codex_protocol::ResponseItemId;
use codex_protocol::models::AgentMessageInputContent;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseItem;

pub fn is_encrypted_subagent_reply(
    author: &str,
    recipient: &str,
    content: &[AgentMessageInputContent],
) -> bool {
    author
        .strip_prefix(recipient)
        .is_some_and(|path| path.starts_with('/'))
        && matches!(
            content.first(),
            Some(AgentMessageInputContent::InputText { .. })
        )
        && content
            .iter()
            .any(|part| matches!(part, AgentMessageInputContent::EncryptedContent { .. }))
        && AgentPath::try_from(author).is_ok()
        && AgentPath::try_from(recipient).is_ok()
}

/// Preserve the part boundaries and opaque ciphertext. A delivered reply has no call
/// in the recipient's history, so give the standalone output a name and no call ID.
pub fn subagent_reply_output(item: &ResponseItem) -> Option<ResponseItem> {
    let ResponseItem::AgentMessage {
        id,
        author,
        recipient,
        content,
        internal_chat_message_metadata_passthrough,
    } = item
    else {
        return None;
    };
    if !is_encrypted_subagent_reply(author, recipient, content) {
        return None;
    }
    let output = content
        .iter()
        .enumerate()
        .map(|(index, part)| match part {
            AgentMessageInputContent::InputText { text } => {
                FunctionCallOutputContentItem::InputText {
                    text: if index == 0 {
                        format!("[assistant:{author} → {recipient}]\n{text}")
                    } else {
                        text.clone()
                    },
                }
            }
            AgentMessageInputContent::EncryptedContent { encrypted_content } => {
                FunctionCallOutputContentItem::EncryptedContent {
                    encrypted_content: encrypted_content.clone(),
                }
            }
        })
        .collect();
    Some(ResponseItem::FunctionCallOutput {
        id: id
            .as_deref()
            .and_then(|id| id.strip_prefix("amsg_"))
            .map(|suffix| ResponseItemId::with_suffix("fco", suffix)),
        call_id: None,
        namespace: Some("guardian".into()),
        name: Some("subagent_reply".into()),
        output: FunctionCallOutputPayload::from_content_items(output),
        internal_chat_message_metadata_passthrough: internal_chat_message_metadata_passthrough
            .clone(),
    })
}
