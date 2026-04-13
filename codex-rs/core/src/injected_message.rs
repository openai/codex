//! Generated user-message payloads and model-visible XML envelopes.
//!
//! This module owns the representation for messages that are delivered into a
//! thread by the harness rather than typed directly by the user. The model sees
//! the XML envelope recorded in history; transcript clients receive a separate
//! structured event with the human-facing text so they do not need to parse or
//! hide that XML.

#![allow(dead_code)]

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum InjectedMessage {
    External {
        source: String,
        content: String,
    },
    Timer {
        timer_id: String,
        content: String,
        instructions: Option<String>,
    },
}

impl InjectedMessage {
    pub(crate) fn from_external_row(
        row: codex_state::ExternalMessage,
    ) -> Result<(Self, TimerDelivery), String> {
        let delivery = serde_json::from_value::<TimerDelivery>(serde_json::Value::String(
            row.delivery.clone(),
        ))
        .map_err(|err| format!("invalid message delivery `{}`: {err}", row.delivery))?;
        Ok((
            Self::External {
                source: row.source,
                content: row.content,
            },
            delivery,
        ))
    }

    pub(crate) fn prompt_input_item(&self) -> ResponseInputItem {
        ResponseInputItem::Message {
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: self.render_prompt(),
            }],
        }
    }

    pub(crate) fn event(&self) -> InjectedMessageEvent {
        match self {
            Self::External { source, content } => InjectedMessageEvent {
                content: content.clone(),
                source: source.clone(),
            },
            Self::Timer {
                timer_id, content, ..
            } => InjectedMessageEvent {
                content: content.clone(),
                source: format!("timer {timer_id}"),
            },
        }
    }

    fn render_prompt(&self) -> String {
        let mut rendered = String::new();
        match self {
            Self::External { content, .. } => {
                rendered.push_str(EXTERNAL_MESSAGE_OPEN_TAG);
                rendered.push('\n');
                push_block_tag(&mut rendered, "content", content);
                rendered.push_str(EXTERNAL_MESSAGE_CLOSE_TAG);
            }
            Self::Timer {
                timer_id,
                content,
                instructions,
            } => {
                rendered.push_str(TIMER_MESSAGE_OPEN_TAG);
                rendered.push('\n');
                push_tag(&mut rendered, "timer_id", timer_id);
                push_block_tag(&mut rendered, "content", content);
                if let Some(instructions) = instructions.as_deref() {
                    push_block_tag(&mut rendered, "instructions", instructions);
                }
                rendered.push_str(TIMER_MESSAGE_CLOSE_TAG);
            }
        }
        rendered
    }
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
        let message = InjectedMessage::External {
            source: "external".to_string(),
            content: "run <tests>".to_string(),
        };

        assert_eq!(
            message.render_prompt(),
            "<external_message>\n<content>\nrun &lt;tests&gt;\n</content>\n</external_message>"
        );
    }

    #[test]
    fn renders_timer_message_prompt_with_timer_id() {
        let message = InjectedMessage::Timer {
            timer_id: "timer-1".to_string(),
            content: "run <tests>".to_string(),
            instructions: Some("stay \"brief\"".to_string()),
        };

        assert_eq!(
            message.render_prompt(),
            "<timer_message>\n<timer_id>timer-1</timer_id>\n<content>\nrun &lt;tests&gt;\n</content>\n<instructions>\nstay &quot;brief&quot;\n</instructions>\n</timer_message>"
        );
    }
}
