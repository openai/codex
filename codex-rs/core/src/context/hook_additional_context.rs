use super::ContextualUserFragment;
use codex_protocol::models::ContentItemKind;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct HookAdditionalContext {
    text: String,
}

impl HookAdditionalContext {
    pub(crate) fn new(text: impl Into<String>) -> Self {
        Self { text: text.into() }
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

    fn body(&self) -> String {
        self.text.clone()
    }
}
