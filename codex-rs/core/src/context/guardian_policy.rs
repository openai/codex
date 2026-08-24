use super::ContextualUserFragment;
use codex_protocol::models::ContentItemKind;

/// The isolated developer policy provided to a Guardian reviewer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GuardianPolicy {
    policy: String,
}

impl GuardianPolicy {
    pub(crate) fn new(policy: impl Into<String>) -> Self {
        Self {
            policy: policy.into(),
        }
    }
}

impl ContextualUserFragment for GuardianPolicy {
    fn content_kind(&self) -> ContentItemKind {
        ContentItemKind("guardian.policy".to_string())
    }

    fn role(&self) -> &'static str {
        "developer"
    }

    fn requires_separate_message(&self) -> bool {
        true
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
