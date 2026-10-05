use super::ContextualUserFragment;
use codex_protocol::models::ContentItemKind;
use codex_protocol::models::ResponseItem;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct BaseInstructionsFragment(pub(crate) String);

impl BaseInstructionsFragment {
    pub(crate) const KIND: &str = "model.base_instructions";

    pub(crate) fn matches_item(item: &ResponseItem) -> bool {
        let ResponseItem::Message {
            internal_chat_message_metadata_passthrough: Some(metadata),
            ..
        } = item
        else {
            return false;
        };
        metadata
            .content_item_kinds
            .as_ref()
            .is_some_and(|kinds| kinds.iter().any(|kind| kind.0 == Self::KIND))
    }
}

impl ContextualUserFragment for BaseInstructionsFragment {
    fn content_kind(&self) -> ContentItemKind {
        ContentItemKind(Self::KIND.to_string())
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
        self.0.clone()
    }
}
