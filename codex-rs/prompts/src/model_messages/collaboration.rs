//! Resolves collaboration catalog text and bundled presets while retaining their source.
//! Initial guidance is catalog-only. Selected modes fall back to their settings.

use super::ResolvedMessage;
use codex_collaboration_mode_templates::DEFAULT;
use codex_collaboration_mode_templates::PLAN;
use codex_protocol::openai_models::CollaborationModeMessages;

#[derive(Debug, Clone, Copy)]
pub struct ResolvedCollaborationModeMessages<'a> {
    pub initial: Option<&'a str>,
    pub default: ResolvedMessage<'a>,
    pub plan: ResolvedMessage<'a>,
}

impl<'a> ResolvedCollaborationModeMessages<'a> {
    pub(crate) fn new(messages: Option<&'a CollaborationModeMessages>) -> Self {
        Self {
            initial: messages.and_then(|m| m.initial.as_deref()),
            default: ResolvedMessage::new(messages.and_then(|m| m.default.as_deref()), DEFAULT),
            plan: ResolvedMessage::new(messages.and_then(|m| m.plan.as_deref()), PLAN),
        }
    }
}
