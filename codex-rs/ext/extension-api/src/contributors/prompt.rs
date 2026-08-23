// All this file should be replaced by the existing fragment implementation ofc

use codex_protocol::models::ContentItemKind;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PromptSlot {
    DeveloperPolicy,
    DeveloperCapabilities,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PromptFragment {
    slot: PromptSlot,
    text: String,
    content_kind: ContentItemKind,
}

impl PromptFragment {
    /// Creates a prompt fragment for the given slot.
    pub fn new(slot: PromptSlot, text: impl Into<String>, content_kind: ContentItemKind) -> Self {
        Self {
            slot,
            text: text.into(),
            content_kind,
        }
    }

    /// Creates a developer-policy prompt fragment.
    pub fn developer_policy(text: impl Into<String>, content_kind: ContentItemKind) -> Self {
        Self::new(PromptSlot::DeveloperPolicy, text, content_kind)
    }

    /// Creates a developer-capabilities prompt fragment.
    pub fn developer_capability(text: impl Into<String>, content_kind: ContentItemKind) -> Self {
        Self::new(PromptSlot::DeveloperCapabilities, text, content_kind)
    }

    /// Returns the target prompt slot.
    pub fn slot(&self) -> PromptSlot {
        self.slot
    }

    /// Returns the model-visible text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Returns the producer-owned classification of the model-visible text.
    pub fn content_kind(&self) -> &ContentItemKind {
        &self.content_kind
    }
}
