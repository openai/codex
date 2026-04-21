use super::ContextualUserFragment;
use codex_protocol::protocol::REALTIME_CONVERSATION_CLOSE_TAG;
use codex_protocol::protocol::REALTIME_CONVERSATION_OPEN_TAG;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RealtimeStartWithInstructions {
    instructions: String,
}

impl RealtimeStartWithInstructions {
    pub(crate) fn new(instructions: impl Into<String>) -> Self {
        Self {
            instructions: instructions.into(),
        }
    }
}

impl ContextualUserFragment for RealtimeStartWithInstructions {
    const ROLE: &'static str = "developer";
    const START_MARKER: &'static str = REALTIME_CONVERSATION_OPEN_TAG;
    const END_MARKER: &'static str = REALTIME_CONVERSATION_CLOSE_TAG;

    fn body(&self) -> String {
        format!("\n{}", self.instructions)
    }
}
