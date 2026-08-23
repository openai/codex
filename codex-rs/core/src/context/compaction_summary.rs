use super::ContextualUserFragment;
use codex_protocol::models::ContentItemKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CompactionSummary {
    summary: String,
}

impl CompactionSummary {
    pub(crate) fn new(summary: impl Into<String>) -> Self {
        Self {
            summary: summary.into(),
        }
    }
}

impl ContextualUserFragment for CompactionSummary {
    fn content_kind(&self) -> ContentItemKind {
        ContentItemKind("compaction.summary".to_string())
    }

    fn role(&self) -> &'static str {
        "user"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        ("", "")
    }

    fn body(&self) -> String {
        self.summary.clone()
    }
}
