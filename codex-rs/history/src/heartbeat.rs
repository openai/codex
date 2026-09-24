//! Identifies host-marked heartbeat inputs and their exact saved instruction bodies.
//! Unknown origins, extra content, oversized inputs and unfamiliar wrappers stay unchanged.

use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;

pub const HEARTBEAT_CONTENT_KIND: &str = "user.heartbeat";

/// Provenance of one accepted input, independent of the active turn's trigger.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum UserInputOrigin {
    /// Ordinary or unclassified input, never eligible for heartbeat coalescing.
    #[default]
    User,
    /// Input submitted by a scheduled heartbeat, including when it steers an active turn.
    Heartbeat,
}

impl UserInputOrigin {
    pub fn from_turn_trigger(trigger: Option<&str>) -> Self {
        match trigger {
            Some("automation_heartbeat_scheduled") => Self::Heartbeat,
            _ => Self::User,
        }
    }

    pub fn from_message(item: &ResponseItem) -> Self {
        match item {
            ResponseItem::Message {
                role,
                internal_chat_message_metadata_passthrough: Some(metadata),
                ..
            } if role == "user"
                && metadata.content_item_kinds.as_deref().is_some_and(|kinds| {
                    kinds.len() == 1 && kinds[0].0 == HEARTBEAT_CONTENT_KIND
                }) =>
            {
                Self::Heartbeat
            }
            _ => Self::User,
        }
    }

    pub fn is_user(&self) -> bool {
        *self == Self::User
    }
}

/// A bounded scheduler envelope. Instruction equality excludes only invocation time.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Heartbeat<'a> {
    pub automation_id: &'a str,
    pub timestamp: &'a str,
    pub instructions: &'a str,
}

impl<'a> Heartbeat<'a> {
    pub fn from_message(item: &'a ResponseItem) -> Option<Self> {
        if UserInputOrigin::from_message(item) != UserInputOrigin::Heartbeat {
            return None;
        }
        let ResponseItem::Message { content, .. } = item else {
            return None;
        };
        let [ContentItem::InputText { text }] = content.as_slice() else {
            return None;
        };
        Self::parse(text)
    }

    pub(crate) fn parse(text: &'a str) -> Option<Self> {
        // Reserve the retained section's order label and its per-line "user: " prefix.
        // Oversized definitions keep their existing representation in every consumer.
        if text
            .len()
            .saturating_add(text.lines().count().saturating_mul(/*rhs*/ 6))
            .saturating_add(/*rhs*/ 64)
            > 3_600
        {
            return None;
        }
        // The scheduler terminates the envelope with a newline; preserve body whitespace.
        let text = text.strip_suffix('\n').unwrap_or(text);
        let text = text.strip_prefix("<heartbeat>\n  <automation_id>")?;
        let (automation_id, text) = text.split_once("</automation_id>\n  <current_time_iso>")?;
        let (timestamp, text) = text.split_once("</current_time_iso>\n  <instructions>\n")?;
        let instructions = text.strip_suffix("\n  </instructions>\n</heartbeat>")?;
        if automation_id.is_empty()
            || automation_id.len() > 128
            || !automation_id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_'))
            || timestamp.is_empty()
            || timestamp.len() > 64
            || !timestamp
                .bytes()
                .all(|c| c.is_ascii_digit() || matches!(c, b'T' | b'Z' | b':' | b'.' | b'+' | b'-'))
        {
            return None;
        }
        Some(Self {
            automation_id,
            timestamp,
            instructions,
        })
    }
}

#[cfg(test)]
#[path = "heartbeat_tests.rs"]
mod tests;
