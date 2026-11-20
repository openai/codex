use super::ContextualUserFragment;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ContentItemKind;
use codex_protocol::models::input_text;

/// Developer note introducing a child after its inherited conversation prefix.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MultiAgentUsageHint {
    text: String,
}

impl MultiAgentUsageHint {
    pub(crate) const KIND: &str = "multi_agent.usage_hint";

    pub(crate) fn new(text: &str) -> Self {
        Self {
            text: text.to_string(),
        }
    }
}

impl ContextualUserFragment for MultiAgentUsageHint {
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

    fn body(&self) -> ContentItem {
        input_text(self.text.clone())
    }
}
