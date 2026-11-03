//! Supplies a bounded, review-scoped request to reconsider a Guardian denial.

use super::ContextualUserFragment;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ContentItemKind;
use codex_protocol::models::input_text;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct GuardianDenialReconsideration<'a> {
    pub(crate) prompt: Option<&'a str>,
}

impl GuardianDenialReconsideration<'_> {
    pub(crate) const MAX_BYTES: usize = 1_000;
}

impl ContextualUserFragment for GuardianDenialReconsideration<'_> {
    fn content_kind(&self) -> ContentItemKind {
        ContentItemKind("guardian.denial_reconsideration".to_owned())
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
        let prompt = self.prompt.unwrap_or(
            "For this review only, reconsider your previous denial of the same planned action. \
             Think carefully about the action, the available evidence, and the user's authorization \
             under the same security policy. This request grants no new authorization and does not \
             change the policy. Return your final assessment using the same JSON schema.",
        );
        // Config loading rejects oversized overrides; also bound programmatic callers.
        input_text(prompt[..prompt.floor_char_boundary(Self::MAX_BYTES)].to_owned())
    }
}
