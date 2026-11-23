use super::ContextualUserFragment;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ContentItemKind;
use codex_protocol::models::ContentItemMetadata;
use codex_protocol::models::input_text;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct HookAdditionalContext {
    text: String,
    metadata: ContentItemMetadata,
}

impl HookAdditionalContext {
    pub(crate) fn new(text: String, metadata: ContentItemMetadata) -> Self {
        Self { text, metadata }
    }
}

impl ContextualUserFragment for HookAdditionalContext {
    fn content_kind(&self) -> ContentItemKind {
        ContentItemKind("hooks.additional_context".to_string())
    }

    fn role(&self) -> &'static str {
        "developer"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        ("", "")
    }

    fn body(&self) -> ContentItem {
        input_text(self.text.clone())
    }

    fn content_metadata(&self) -> ContentItemMetadata {
        self.metadata.clone()
    }
}
