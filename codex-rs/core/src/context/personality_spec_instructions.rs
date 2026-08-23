use super::ContextualUserFragment;
use codex_protocol::models::ContentItemKind;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PersonalitySpecInstructions {
    spec: String,
}

impl PersonalitySpecInstructions {
    pub(crate) fn new(spec: impl Into<String>) -> Self {
        Self { spec: spec.into() }
    }
}

impl ContextualUserFragment for PersonalitySpecInstructions {
    fn content_kind(&self) -> ContentItemKind {
        ContentItemKind("personality.spec_instructions".to_string())
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
        ("<personality_spec>", "</personality_spec>")
    }

    fn body(&self) -> String {
        format!(
            " The user has requested a new communication style. Future messages should adhere to the following personality: \n{} ",
            self.spec
        )
    }
}
