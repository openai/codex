use codex_protocol::models::ContentItem;
use codex_protocol::models::ContentItemKind;
use codex_protocol::models::ContentItemMetadata;
use codex_protocol::models::ResponseItem;

/// Content, classification and attribution travel together through edits and truncation.
#[derive(Clone, Debug, PartialEq)]
pub struct AnnotatedContent {
    content: ContentItem,
    kind: ContentItemKind,
    metadata: ContentItemMetadata,
}

impl AnnotatedContent {
    /// Creates content and its classification together.
    pub fn new(content: ContentItem, kind: ContentItemKind) -> Self {
        Self {
            content,
            kind,
            metadata: ContentItemMetadata::default(),
        }
    }

    /// Creates text with the attribution supplied by its producer.
    pub fn text(
        text: impl Into<String>,
        kind: ContentItemKind,
        metadata: ContentItemMetadata,
    ) -> Self {
        Self {
            content: ContentItem::InputText { text: text.into() },
            kind,
            metadata,
        }
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

    /// Separates content and its positional annotations at an API boundary.
    pub fn into_parts(self) -> (ContentItem, ContentItemKind, ContentItemMetadata) {
        (self.content, self.kind, self.metadata)
    }
}

/// Assembles a message without separating its content from its attribution.
pub fn message_from_parts(role: impl Into<String>, parts: Vec<AnnotatedContent>) -> ResponseItem {
    let mut item = ResponseItem::Message {
        id: None,
        role: role.into(),
        content: Vec::new(),
        phase: None,
        internal_chat_message_metadata_passthrough: None,
    };
    set_annotated_content(&mut item, parts);
    item
}

/// Takes a message's content together with its classification and source metadata.
///
/// Legacy messages, including persisted rollouts, may not have classifications.
/// Missing entries have unknown classification and source so the message remains usable.
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
    let metadata = internal_chat_message_metadata_passthrough
        .as_mut()
        .and_then(|metadata| metadata.content_item_metadata.take())
        .unwrap_or_default();

    Some(
        std::mem::take(content)
            .into_iter()
            .zip(kinds.into_iter().chain(std::iter::repeat_with(|| {
                ContentItemKind("unknown".to_string())
            })))
            .zip(
                metadata
                    .into_iter()
                    .chain(std::iter::repeat_with(ContentItemMetadata::default)),
            )
            .map(|((content, kind), metadata)| AnnotatedContent {
                content,
                kind,
                metadata,
            })
            .collect(),
    )
}

/// Replaces content and its positional annotations together.
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

    let mut updated_content = Vec::with_capacity(annotated_content.len());
    let mut content_item_kinds = Vec::with_capacity(annotated_content.len());
    let mut content_item_metadata = Vec::with_capacity(annotated_content.len());
    for annotated in annotated_content {
        let (content, kind, metadata) = annotated.into_parts();
        updated_content.push(content);
        content_item_kinds.push(kind);
        content_item_metadata.push(metadata);
    }
    *content = updated_content;
    let metadata = internal_chat_message_metadata_passthrough.get_or_insert_default();
    metadata.content_item_kinds = Some(content_item_kinds);
    metadata.content_item_metadata = content_item_metadata
        .iter()
        .any(|metadata| metadata != &ContentItemMetadata::default())
        .then_some(content_item_metadata);

    Some(())
}
