//! Generated user-message payloads and model-visible XML envelopes.
//!
//! This module owns the shared representation for messages that are delivered
//! into a thread by the harness rather than typed directly by the user. Today
//! that includes external queued messages and timer-generated messages. The
//! state database keeps the richer persisted payload, while this module renders
//! the smaller XML envelope that is recorded in model history and builds the
//! structured display event that clients can render without parsing that XML.

use crate::timers::TimerDelivery;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::protocol::InjectedMessageEvent;
use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeMap;

const EXTERNAL_MESSAGE_OPEN_TAG: &str = "<external_message>";
const EXTERNAL_MESSAGE_CLOSE_TAG: &str = "</external_message>";
const TIMER_MESSAGE_OPEN_TAG: &str = "<timer_message>";
const TIMER_MESSAGE_CLOSE_TAG: &str = "</timer_message>";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessagePayload {
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    #[serde(default)]
    pub meta: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ThreadMessage {
    pub id: String,
    pub thread_id: String,
    pub source: String,
    pub content: String,
    pub instructions: Option<String>,
    pub meta: BTreeMap<String, String>,
    pub delivery: TimerDelivery,
    pub queued_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum MessageInvocationKind {
    External,
    Timer { timer_id: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MessageInvocationContext {
    pub(crate) kind: MessageInvocationKind,
    pub(crate) source: String,
    pub(crate) content: String,
    pub(crate) instructions: Option<String>,
}

pub fn validate_meta_key(key: &str) -> Result<(), String> {
    if key.is_empty() {
        return Err("message metadata keys cannot be empty".to_string());
    }
    if key
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    {
        return Ok(());
    }
    Err(format!(
        "message metadata key `{key}` must contain only letters, digits, and underscores"
    ))
}

pub fn validate_meta(meta: &BTreeMap<String, String>) -> Result<(), String> {
    for key in meta.keys() {
        validate_meta_key(key)?;
    }
    Ok(())
}

pub(crate) fn message_prompt_input_item(message: &MessageInvocationContext) -> ResponseInputItem {
    ResponseInputItem::Message {
        role: "user".to_string(),
        content: vec![ContentItem::InputText {
            text: render_message_prompt(message),
        }],
    }
}

pub(crate) fn injected_message_event(message: &MessageInvocationContext) -> InjectedMessageEvent {
    InjectedMessageEvent {
        content: message.content.clone(),
        source: message.source.clone(),
    }
}

pub(crate) fn render_message_prompt(message: &MessageInvocationContext) -> String {
    let mut rendered = String::new();
    match &message.kind {
        MessageInvocationKind::External => {
            rendered.push_str(EXTERNAL_MESSAGE_OPEN_TAG);
            rendered.push('\n');
            push_block_tag(&mut rendered, "content", &message.content);
            rendered.push_str(EXTERNAL_MESSAGE_CLOSE_TAG);
        }
        MessageInvocationKind::Timer { timer_id } => {
            rendered.push_str(TIMER_MESSAGE_OPEN_TAG);
            rendered.push('\n');
            push_tag(&mut rendered, "timer_id", timer_id);
            push_block_tag(&mut rendered, "content", &message.content);
            if let Some(instructions) = message.instructions.as_deref() {
                push_block_tag(&mut rendered, "instructions", instructions);
            }
            rendered.push_str(TIMER_MESSAGE_CLOSE_TAG);
        }
    }
    rendered
}

pub(crate) fn db_message_to_thread_message(
    row: codex_state::ThreadMessage,
) -> Result<ThreadMessage, String> {
    let delivery =
        serde_json::from_value::<TimerDelivery>(serde_json::Value::String(row.delivery.clone()))
            .map_err(|err| format!("invalid message delivery `{}`: {err}", row.delivery))?;
    let meta = serde_json::from_str::<BTreeMap<String, String>>(&row.meta_json)
        .map_err(|err| format!("invalid message metadata json for {}: {err}", row.id))?;
    validate_meta(&meta)?;
    Ok(ThreadMessage {
        id: row.id,
        thread_id: row.thread_id,
        source: row.source,
        content: row.content,
        instructions: row.instructions,
        meta,
        delivery,
        queued_at: row.queued_at,
    })
}

fn push_tag(rendered: &mut String, tag: &str, value: &str) {
    rendered.push('<');
    rendered.push_str(tag);
    rendered.push('>');
    rendered.push_str(&xml_escape(value));
    rendered.push_str("</");
    rendered.push_str(tag);
    rendered.push_str(">\n");
}

fn push_block_tag(rendered: &mut String, tag: &str, value: &str) {
    rendered.push('<');
    rendered.push_str(tag);
    rendered.push_str(">\n");
    rendered.push_str(&xml_escape(value));
    rendered.push('\n');
    rendered.push_str("</");
    rendered.push_str(tag);
    rendered.push_str(">\n");
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn renders_external_message_prompt_with_only_content() {
        let message = MessageInvocationContext {
            kind: MessageInvocationKind::External,
            source: "external".to_string(),
            content: "run <tests>".to_string(),
            instructions: Some("stay \"brief\"".to_string()),
        };

        assert_eq!(
            render_message_prompt(&message),
            "<external_message>\n<content>\nrun &lt;tests&gt;\n</content>\n</external_message>"
        );
    }

    #[test]
    fn renders_timer_message_prompt_with_timer_id_and_no_metadata() {
        let message = MessageInvocationContext {
            kind: MessageInvocationKind::Timer {
                timer_id: "timer-1".to_string(),
            },
            source: "timer timer-1".to_string(),
            content: "run <tests>".to_string(),
            instructions: Some("stay \"brief\"".to_string()),
        };

        assert_eq!(
            render_message_prompt(&message),
            "<timer_message>\n<timer_id>timer-1</timer_id>\n<content>\nrun &lt;tests&gt;\n</content>\n<instructions>\nstay &quot;brief&quot;\n</instructions>\n</timer_message>"
        );
    }

    #[test]
    fn validate_meta_key_rejects_hyphens() {
        assert_eq!(
            validate_meta_key("bad-key"),
            Err(
                "message metadata key `bad-key` must contain only letters, digits, and underscores"
                    .to_string()
            )
        );
    }
}
