//! Extension prompt placement with content and producer attribution kept together.

use codex_context_fragments::AnnotatedContent;
use codex_context_fragments::RenderedFragment;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ContentItemKind;
use codex_protocol::models::ContentItemMetadata;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PromptSlot {
    DeveloperPolicy,
    DeveloperCapabilities,
    /// Text inside the context-window message, supplied by `contribute_thread_context`.
    ContextWindow,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PromptFragment {
    slot: PromptSlot,
    content: AnnotatedContent,
}

impl PromptFragment {
    /// Creates harness-provided context for the given slot.
    pub fn new(slot: PromptSlot, text: impl Into<String>, content_kind: ContentItemKind) -> Self {
        Self {
            slot,
            content: AnnotatedContent::text(text, content_kind, ContentItemMetadata::harness()),
        }
    }

    /// Creates tool-supplied context without changing its prompt placement.
    pub fn tool(
        slot: PromptSlot,
        text: impl Into<String>,
        content_kind: ContentItemKind,
        namespace: String,
    ) -> Self {
        Self {
            slot,
            content: AnnotatedContent::text(
                text,
                content_kind,
                ContentItemMetadata::tool(Some(namespace.into())),
            ),
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
        let ContentItem::InputText { text } = self.content.content() else {
            unreachable!("prompt fragments contain text");
        };
        text
    }

    /// Returns the producer-owned classification of the model-visible text.
    pub fn content_kind(&self) -> &ContentItemKind {
        self.content.kind()
    }

    /// Consumes the placement wrapper when assembling a shared context message.
    pub fn into_content(self) -> AnnotatedContent {
        self.content
    }
}

impl From<PromptFragment> for RenderedFragment {
    fn from(fragment: PromptFragment) -> Self {
        Self::new("developer", fragment.content)
    }
}
