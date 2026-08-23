use super::ContextualUserFragment;
use codex_protocol::models::ContentItemKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct GuardianNodeReplPolicy;

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
        include_str!("../guardian/node_repl_policy.md").to_string()
    }
}
