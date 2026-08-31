use super::ContextualUserFragment;
use codex_protocol::models::ContentItemKind;
use codex_protocol::openai_models::ModelMessages;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GuardianNodeReplPolicy {
    policy: String,
}

impl GuardianNodeReplPolicy {
    pub(crate) fn from_model_messages(messages: Option<&ModelMessages>) -> Self {
        let policy = messages
            .and_then(|messages| messages.auto_review.as_ref())
            .and_then(|messages| messages.node_repl_policy.as_deref())
            .unwrap_or(include_str!("../../assets/guardian/node_repl_policy.md"));
        Self {
            policy: policy.to_string(),
        }
    }
}

impl ContextualUserFragment for GuardianNodeReplPolicy {
    fn content_kind(&self) -> ContentItemKind {
        ContentItemKind("guardian.node_repl_policy".to_string())
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
        self.policy.clone()
    }
}
