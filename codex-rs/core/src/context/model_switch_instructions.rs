use super::ContextualUserFragment;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ModelSwitchInstructions {
    model_instructions: String,
}

impl ModelSwitchInstructions {
    pub(crate) fn new(model_instructions: impl Into<String>) -> Self {
        Self {
            model_instructions: model_instructions.into(),
        }
    }
}

impl ContextualUserFragment for ModelSwitchInstructions {
    const ROLE: &'static str = "developer";
    const START_MARKER: &'static str = "<model_switch>";
    const END_MARKER: &'static str = "</model_switch>";

    fn body(&self) -> String {
        format!(
            "\nThe user was previously using a different model. Please continue the conversation according to the following instructions:\n\n{}\n",
            self.model_instructions
        )
    }
}
