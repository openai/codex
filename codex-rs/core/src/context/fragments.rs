use super::ContextualUserFragment;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseInputItem;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AdditionalContextFragment {
    pub(crate) key: String,
    pub(crate) value: String,
}

impl AdditionalContextFragment {
    const END_MARKER_SUFFIX: &'static str = ">";
    const START_MARKER_PREFIX: &'static str = "<external_";

    pub(crate) fn new(key: String, value: String) -> Self {
        Self { key, value }
    }

    pub(crate) fn input_item(fragments: Vec<Self>) -> Option<ResponseInputItem> {
        let content = fragments
            .into_iter()
            .map(|fragment| ContentItem::InputText {
                text: fragment.render(),
            })
            .collect::<Vec<_>>();
        if content.is_empty() {
            return None;
        }

        Some(ResponseInputItem::Message {
            role: Self::role().to_string(),
            content,
            phase: None,
        })
    }
}

impl ContextualUserFragment for AdditionalContextFragment {
    fn role() -> &'static str {
        "user"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        (Self::START_MARKER_PREFIX, Self::END_MARKER_SUFFIX)
    }

    fn body(&self) -> String {
        format!("{}>{}</external_{}", self.key, self.value, self.key)
    }
}
