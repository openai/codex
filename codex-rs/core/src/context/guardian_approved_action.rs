use super::ContextualUserFragment;
use crate::guardian::AUTO_REVIEW_DENIED_ACTION_APPROVAL_DEVELOPER_PREFIX;
use codex_guardian_context::MANUAL_APPROVAL_CONTENT_KIND;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ContentItemKind;
use codex_protocol::models::input_text;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GuardianApprovedAction {
    approved_action_json: String,
}

impl GuardianApprovedAction {
    pub(crate) const KIND: &str = MANUAL_APPROVAL_CONTENT_KIND;

    pub(crate) fn new(approved_action_json: String) -> Self {
        Self {
            approved_action_json,
        }
    }
}

impl ContextualUserFragment for GuardianApprovedAction {
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
        let approval_prefix = AUTO_REVIEW_DENIED_ACTION_APPROVAL_DEVELOPER_PREFIX;
        let approved_action_json = &self.approved_action_json;
        input_text(format!(
            r#"{approval_prefix}

Treat this as approval to perform that exact action in the same context in which it was originally requested.
Do not assume this also authorizes similar operations with different payloads.

Approved action:
{approved_action_json}"#
        ))
    }
}
