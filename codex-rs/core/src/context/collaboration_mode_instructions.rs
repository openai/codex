use super::ContextualUserFragment;
use codex_protocol::config_types::CollaborationMode;
use codex_protocol::protocol::COLLABORATION_MODE_CLOSE_TAG;
use codex_protocol::protocol::COLLABORATION_MODE_OPEN_TAG;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CollaborationModeInstructions {
    instructions: String,
}

impl CollaborationModeInstructions {
    pub(crate) fn from_collaboration_mode(collaboration_mode: &CollaborationMode) -> Option<Self> {
        collaboration_mode
            .settings
            .developer_instructions
            .as_ref()
            .filter(|instructions| !instructions.is_empty())
            .map(|instructions| Self {
                instructions: instructions.clone(),
            })
    }
}

impl ContextualUserFragment for CollaborationModeInstructions {
    const ROLE: &'static str = "developer";
    const START_MARKER: &'static str = COLLABORATION_MODE_OPEN_TAG;
    const END_MARKER: &'static str = COLLABORATION_MODE_CLOSE_TAG;

    fn body(&self) -> String {
        self.instructions.clone()
    }
}
