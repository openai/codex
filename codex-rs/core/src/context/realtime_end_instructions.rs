use super::ContextualUserFragment;
use codex_prompts::END_INSTRUCTIONS;
use codex_protocol::protocol::REALTIME_CONVERSATION_CLOSE_TAG;
use codex_protocol::protocol::REALTIME_CONVERSATION_OPEN_TAG;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RealtimeEndInstructions {
    instructions: String,
    reason: String,
}

impl RealtimeEndInstructions {
    pub(crate) fn new(reason: impl Into<String>) -> Self {
        Self::with_instructions(reason, END_INSTRUCTIONS.trim())
    }

    pub(crate) fn with_instructions(
        reason: impl Into<String>,
        instructions: impl Into<String>,
    ) -> Self {
        Self {
            instructions: instructions.into(),
            reason: reason.into(),
        }
    }
}

impl ContextualUserFragment for RealtimeEndInstructions {
    fn role(&self) -> &'static str {
        "developer"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        (
            REALTIME_CONVERSATION_OPEN_TAG,
            REALTIME_CONVERSATION_CLOSE_TAG,
        )
    }

    fn body(&self) -> String {
        format!("\n{}\n\nReason: {}\n", self.instructions, self.reason)
    }
}
