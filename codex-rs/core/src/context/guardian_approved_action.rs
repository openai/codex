use super::ContextualUserFragment;
use crate::guardian::AUTO_REVIEW_DENIED_ACTION_APPROVAL_DEVELOPER_PREFIX;
use codex_protocol::models::ContentItemKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GuardianApprovedAction {
    approved_action_json: String,
}

impl GuardianApprovedAction {
    pub(crate) fn new(approved_action_json: String) -> Self {
        Self {
            approved_action_json,
        }
    }
}

impl ContextualUserFragment for GuardianApprovedAction {
    fn content_kind(&self) -> ContentItemKind {
        ContentItemKind("guardian.approved_action".to_string())
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
        let approval_prefix = AUTO_REVIEW_DENIED_ACTION_APPROVAL_DEVELOPER_PREFIX;
        let approved_action_json = &self.approved_action_json;
        format!(
            r#"{approval_prefix}

Treat this as approval to perform that exact action in the same context in which it was originally requested.
Do not assume this also authorizes similar operations with different payloads.

Approved action:
{approved_action_json}"#
        )
    }
}
