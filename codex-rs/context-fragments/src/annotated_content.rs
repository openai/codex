use codex_protocol::models::ContentItem;
use codex_protocol::models::ContentItemKind;

/// Model-visible content paired with its harness-owned classification.
#[derive(Clone, Debug, PartialEq)]
pub struct AnnotatedContent {
    content: ContentItem,
    kind: ContentItemKind,
}

impl AnnotatedContent {
    /// Creates content and its classification together.
    pub fn new(content: ContentItem, kind: ContentItemKind) -> Self {
        Self { content, kind }
    }

    /// Creates model-visible input text and its classification together.
    pub fn input_text(text: impl Into<String>, kind: ContentItemKind) -> Self {
        Self::new(ContentItem::InputText { text: text.into() }, kind)
    }

    /// Returns the model-visible content.
    pub fn content(&self) -> &ContentItem {
        &self.content
    }

    /// Returns the classification associated with the content.
    pub fn kind(&self) -> &ContentItemKind {
        &self.kind
    }

    /// Separates the content from its classification at an API boundary.
    pub fn into_parts(self) -> (ContentItem, ContentItemKind) {
        (self.content, self.kind)
    }
}
