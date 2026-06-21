use super::WorldStateSection;
use crate::context::CollaborationModeInstructions;
use crate::context::ContextualUserFragment;
use crate::session::turn_context::TurnContext;
use codex_protocol::config_types::CollaborationMode;
use codex_protocol::protocol::TurnContextItem;

#[derive(Debug)]
pub(crate) struct CollaborationModeState {
    mode: Option<CollaborationMode>,
    enabled: bool,
}

impl CollaborationModeState {
    pub(crate) fn from_turn_context(turn_context: &TurnContext) -> Self {
        Self {
            mode: Some(turn_context.collaboration_mode.clone()),
            enabled: turn_context.config.include_collaboration_mode_instructions,
        }
    }

    pub(crate) fn from_turn_context_item(turn_context_item: &TurnContextItem) -> Self {
        Self {
            mode: turn_context_item.collaboration_mode.clone(),
            enabled: false,
        }
    }
}

impl WorldStateSection for CollaborationModeState {
    fn render_diff(&self, previous: Option<&Self>) -> Option<Box<dyn ContextualUserFragment>> {
        if !self.enabled || previous.is_some_and(|previous| self.mode == previous.mode) {
            return None;
        }
        CollaborationModeInstructions::from_collaboration_mode(self.mode.as_ref()?)
            .map(|instructions| Box::new(instructions) as Box<dyn ContextualUserFragment>)
    }
}
