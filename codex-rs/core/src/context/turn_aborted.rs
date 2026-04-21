use super::ContextualUserFragment;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TurnAborted {
    pub(crate) guidance: String,
}

impl TurnAborted {
    pub(crate) const INTERRUPTED_GUIDANCE: &'static str = "The user interrupted the previous turn on purpose. Any running unified exec processes may still be running in the background. If any tools/commands were aborted, they may have partially executed.";

    pub(crate) fn new(guidance: impl Into<String>) -> Self {
        Self {
            guidance: guidance.into(),
        }
    }
}

impl ContextualUserFragment for TurnAborted {
    const ROLE: &'static str = "user";
    const START_MARKER: &'static str = "<turn_aborted>";
    const END_MARKER: &'static str = "</turn_aborted>";

    fn body(&self) -> String {
        format!("\n{}", self.guidance)
    }
}
