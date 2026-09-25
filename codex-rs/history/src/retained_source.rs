//! Host-owned identity and revision of original evidence. Prompt text cannot create delivery proof.
//! Revisions live with retained records; completeness is invalidated when a copy is shortened.

use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;

use crate::RetainedContext;
use crate::RetainedContextEntry;
use crate::RetainedUserMessage;

use super::Ordered;

/// The original message, including its role and turn namespace.
#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RetainedSourceId {
    pub message_id: String,
    pub turn_id: String,
    pub role: RetainedSourceRole,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RetainedSourceRole {
    User,
    Assistant,
}

/// A host-observed version, never a fingerprint of rendered prompt text.
#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RetainedSource {
    pub id: RetainedSourceId,
    /// A fresh opaque version prevents reuse across history resets and forks.
    pub revision: codex_protocol::ResponseItemId,
    pub complete: bool,
}

impl Ordered<RetainedUserMessage> {
    pub(super) fn source(&self, role: RetainedSourceRole) -> Option<RetainedSource> {
        Some(RetainedSource {
            id: RetainedSourceId {
                message_id: self.value.message_id.clone()?,
                turn_id: self.value.turn_id.clone(),
                role,
            },
            revision: self.revision.clone()?,
            complete: self.value.complete,
        })
    }
}

impl RetainedContext {
    /// Restores a host-captured version while replaying its original message.
    /// Live recording must mint a new revision for changed evidence instead.
    pub fn restore_source_revision(&mut self, source: &RetainedSource) -> bool {
        let entries = match source.id.role {
            RetainedSourceRole::User => &mut self.user_messages,
            RetainedSourceRole::Assistant => &mut self.assistant_messages,
        };
        let Some(entry) = entries.iter_mut().find(|entry| {
            entry.value.message_id.as_deref() == Some(source.id.message_id.as_str())
                && entry.value.turn_id == source.id.turn_id
                && entry.value.complete == source.complete
        }) else {
            return false;
        };
        entry.revision = Some(source.revision.clone());
        true
    }

    /// Missing legacy revisions or message IDs cannot establish delivery.
    pub fn source(&self, entry: RetainedContextEntry<'_>) -> Option<RetainedSource> {
        let (message, role, entries) = match entry {
            RetainedContextEntry::UserMessage(message) => {
                (message, RetainedSourceRole::User, &self.user_messages)
            }
            RetainedContextEntry::AssistantMessage(message) => (
                message,
                RetainedSourceRole::Assistant,
                &self.assistant_messages,
            ),
            RetainedContextEntry::VerifiedAnswer(_) => return None,
        };
        entries
            .iter()
            .find(|entry| std::ptr::eq(&entry.value, message))?
            .source(role)
    }
}
