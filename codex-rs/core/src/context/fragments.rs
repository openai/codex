use super::ContextualUserFragment;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseInputItem;
use codex_utils_string::truncate_middle_with_token_budget;

const MAX_ADDITIONAL_CONTEXT_VALUE_TOKENS: usize = 1_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AdditionalContextFragment {
    pub(crate) key: String,
    pub(crate) value: String,
    pub(crate) is_untrusted: bool,
}

impl AdditionalContextFragment {
    const END_MARKER_SUFFIX: &'static str = ">";
    const START_MARKER_PREFIX: &'static str = "<external_";

    pub(crate) fn new(key: String, value: String, is_untrusted: bool) -> Self {
        Self {
            key,
            value,
            is_untrusted,
        }
    }

    pub(crate) fn input_items(fragments: Vec<Self>) -> Vec<ResponseInputItem> {
        fragments
            .into_iter()
            .map(|fragment| ResponseInputItem::Message {
                role: fragment.role().to_string(),
                content: vec![ContentItem::InputText {
                    text: fragment.render(),
                }],
                phase: None,
            })
            .collect()
    }

    fn role(&self) -> &'static str {
        if self.is_untrusted {
            "user"
        } else {
            "developer"
        }
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
        let value =
            truncate_middle_with_token_budget(&self.value, MAX_ADDITIONAL_CONTEXT_VALUE_TOKENS).0;
        format!("{}>{value}</external_{}", self.key, self.key)
    }
}
