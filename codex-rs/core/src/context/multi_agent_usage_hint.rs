use super::ContextualUserFragment;

/// Configured multi-agent instructions emitted as a standalone developer message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MultiAgentUsageHint {
    text: String,
}

impl MultiAgentUsageHint {
    pub(crate) fn new(text: &str) -> Self {
        Self {
            text: text.to_string(),
        }
    }
}

impl ContextualUserFragment for MultiAgentUsageHint {
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
        self.text.clone()
    }
}
