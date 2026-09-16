//! Resolves collaboration catalog text and bundled presets while retaining their source.
//! Runtime consumers use catalog overrides first, then the selected mode's settings.

use super::ResolvedMessage;
use codex_collaboration_mode_templates::DEFAULT;
use codex_collaboration_mode_templates::PLAN;
use codex_protocol::openai_models::CollaborationModeMessages;

#[derive(Debug, Clone, Copy)]
pub struct ResolvedCollaborationModeMessages<'a> {
    pub default: ResolvedMessage<'a>,
    pub plan: ResolvedMessage<'a>,
}

impl<'a> ResolvedCollaborationModeMessages<'a> {
    pub(crate) fn new(messages: Option<&'a CollaborationModeMessages>) -> Self {
        Self {
            default: ResolvedMessage::new(messages.and_then(|m| m.default.as_deref()), DEFAULT),
            plan: ResolvedMessage::new(messages.and_then(|m| m.plan.as_deref()), PLAN),
        }
    }
}
