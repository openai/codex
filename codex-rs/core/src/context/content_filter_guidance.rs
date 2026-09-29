//! Bounded recovery guidance for a response blocked by an unspecified content filter.

use super::ContextualUserFragment;
use codex_protocol::models::ContentItemKind;

pub(crate) struct ContentFilterGuidance {
    pub(crate) text: String,
}

impl ContextualUserFragment for ContentFilterGuidance {
    fn content_kind(&self) -> ContentItemKind {
        ContentItemKind("generic.content_filter_guidance".to_string())
    }

    fn role(&self) -> &'static str {
        "developer"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        ("<content_filter_guidance>", "</content_filter_guidance>")
    }

    fn body(&self) -> String {
        format!("\n{}\n", self.text)
    }
}
