use super::ContextualUserFragment;
use codex_protocol::models::ContentItemKind;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct BaseInstructionsFragment(pub(crate) String);

impl ContextualUserFragment for BaseInstructionsFragment {
    fn content_kind(&self) -> ContentItemKind {
        ContentItemKind("model.base_instructions".to_string())
    }

    fn role(&self) -> &'static str {
        "developer"
    }

    fn requires_separate_message(&self) -> bool {
        true
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        ("", "")
    }

    fn body(&self) -> String {
        self.0.clone()
    }
}
