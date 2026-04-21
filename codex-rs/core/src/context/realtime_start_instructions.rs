use super::ContextualUserFragment;
use codex_protocol::protocol::REALTIME_CONVERSATION_CLOSE_TAG;
use codex_protocol::protocol::REALTIME_CONVERSATION_OPEN_TAG;

const REALTIME_START_INSTRUCTIONS: &str = include_str!("prompts/realtime/realtime_start.md");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RealtimeStartInstructions;

impl ContextualUserFragment for RealtimeStartInstructions {
    const ROLE: &'static str = "developer";
    const START_MARKER: &'static str = REALTIME_CONVERSATION_OPEN_TAG;
    const END_MARKER: &'static str = REALTIME_CONVERSATION_CLOSE_TAG;

    fn body(&self) -> String {
        format!("\n{}\n", REALTIME_START_INSTRUCTIONS.trim())
    }
}
