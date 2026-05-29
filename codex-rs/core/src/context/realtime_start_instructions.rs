use super::ContextualUserFragment;
use codex_protocol::protocol::REALTIME_CONVERSATION_CLOSE_TAG;
use codex_protocol::protocol::REALTIME_CONVERSATION_OPEN_TAG;

const REALTIME_START_INSTRUCTIONS: &str = include_str!("prompts/realtime/realtime_start.md");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RealtimeStartInstructions;

impl ContextualUserFragment for RealtimeStartInstructions {
    fn role() -> &'static str {
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
        format!("\n{}\n", REALTIME_START_INSTRUCTIONS.trim())
    }
}
