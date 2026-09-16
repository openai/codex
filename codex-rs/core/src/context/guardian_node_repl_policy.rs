use super::ContextualUserFragment;
use codex_prompts::ResolvedModelMessages;
use codex_protocol::models::ContentItemKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GuardianNodeReplPolicy {
    policy: String,
}

impl GuardianNodeReplPolicy {
    pub(crate) fn from_messages(model_messages: ResolvedModelMessages<'_>) -> Self {
        let policy = model_messages.auto_review().node_repl_policy;
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
