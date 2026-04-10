//! Queued thread messages and model-visible message envelopes.

use crate::timers::TimerDelivery;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::protocol::InjectedMessageEvent;
use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeMap;

pub const CODEX_MESSAGE_OPEN_TAG: &str = "<codex_message>";
pub const CODEX_MESSAGE_CLOSE_TAG: &str = "</codex_message>";

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
pub(crate) struct MessageInvocationContext {
    pub(crate) source: String,
    pub(crate) content: String,
    pub(crate) instructions: Option<String>,
    pub(crate) meta: BTreeMap<String, String>,
    pub(crate) queued_at: i64,
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
    rendered.push_str(CODEX_MESSAGE_OPEN_TAG);
    rendered.push('\n');
    push_tag(&mut rendered, "source", &message.source);
    push_tag(&mut rendered, "queued_at", &message.queued_at.to_string());
    push_block_tag(&mut rendered, "content", &message.content);
    if let Some(instructions) = message.instructions.as_deref() {
        push_block_tag(&mut rendered, "instructions", instructions);
    }
    if message.meta.is_empty() {
        rendered.push_str("<meta />\n");
    } else {
        rendered.push_str("<meta>\n");
        for (key, value) in &message.meta {
            rendered.push_str("  <entry id=\"");
            rendered.push_str(&xml_escape(key));
            rendered.push_str("\">");
            rendered.push_str(&xml_escape(value));
            rendered.push_str("</entry>\n");
        }
        rendered.push_str("</meta>\n");
    }
    rendered.push_str(CODEX_MESSAGE_CLOSE_TAG);
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
    fn renders_message_prompt_with_escaped_metadata() {
        let message = MessageInvocationContext {
            source: "timer timer-1".to_string(),
            content: "run <tests>".to_string(),
            instructions: Some("stay \"brief\"".to_string()),
            meta: BTreeMap::from([("ticket".to_string(), "ABC&123".to_string())]),
            queued_at: 100,
        };

        assert_eq!(
            render_message_prompt(&message),
            "<codex_message>\n<source>timer timer-1</source>\n<queued_at>100</queued_at>\n<content>\nrun &lt;tests&gt;\n</content>\n<instructions>\nstay &quot;brief&quot;\n</instructions>\n<meta>\n  <entry id=\"ticket\">ABC&amp;123</entry>\n</meta>\n</codex_message>"
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
