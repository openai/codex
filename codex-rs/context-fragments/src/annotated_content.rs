use codex_protocol::models::ContentItem;
use codex_protocol::models::ContentItemKind;
use codex_protocol::models::ResponseItem;

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

    /// Returns the model-visible content for an in-place update.
    pub fn content_mut(&mut self) -> &mut ContentItem {
        &mut self.content
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

/// Takes a message's content together with its positional classifications.
///
/// Legacy messages, including persisted rollouts, may not have classifications.
/// Missing entries are classified as unknown so the message remains usable.
pub fn to_annotated_content(item: &mut ResponseItem) -> Option<Vec<AnnotatedContent>> {
    let ResponseItem::Message {
        content,
        internal_chat_message_metadata_passthrough,
        ..
    } = item
    else {
        return None;
    };

    let kinds = internal_chat_message_metadata_passthrough
        .as_mut()
        .and_then(|metadata| metadata.content_item_kinds.take())
        .unwrap_or_default();

    Some(
        std::mem::take(content)
            .into_iter()
            .zip(kinds.into_iter().chain(std::iter::repeat_with(|| {
                ContentItemKind("unknown".to_string())
            })))
            .map(|(content, kind)| AnnotatedContent::new(content, kind))
            .collect(),
    )
}

/// Replaces a message's content and positional classifications together.
pub fn set_annotated_content(
    item: &mut ResponseItem,
    annotated_content: Vec<AnnotatedContent>,
) -> Option<()> {
    let ResponseItem::Message {
        content,
        internal_chat_message_metadata_passthrough,
        ..
    } = item
    else {
        return None;
    };

    let (updated_content, content_item_kinds): (Vec<_>, Vec<_>) = annotated_content
        .into_iter()
        .map(AnnotatedContent::into_parts)
        .unzip();
    *content = updated_content;
    internal_chat_message_metadata_passthrough
        .get_or_insert_default()
        .content_item_kinds = Some(content_item_kinds);

    Some(())
}
