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
    const ROLE: &'static str = "developer";
    const START_MARKER: &'static str = "<personality_spec>";
    const END_MARKER: &'static str = "</personality_spec>";

    fn body(&self) -> String {
        format!(
            " The user has requested a new communication style. Future messages should adhere to the following personality: \n{} ",
            self.spec
        )
    }
}
