use super::ContextualUserFragment;

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
    fn role() -> &'static str {
        "developer"
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
