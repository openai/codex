use super::WorldStateSection;
use super::developer_message;
use crate::context::CollaborationModeInstructions;
use crate::context::ContextualUserFragment;
use crate::session::turn_context::TurnContext;
use codex_protocol::config_types::CollaborationMode;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::TurnContextItem;

#[derive(Debug, Default)]
pub(crate) struct CollaborationModeState {
    baseline: Option<Option<CollaborationMode>>,
    enabled: bool,
}

impl CollaborationModeState {
    pub(crate) fn from_turn_context(turn_context: &TurnContext) -> Self {
        Self {
            baseline: Some(Some(turn_context.collaboration_mode.clone())),
            enabled: turn_context.config.include_collaboration_mode_instructions,
        }
    }

    pub(crate) fn from_turn_context_item(turn_context_item: &TurnContextItem) -> Self {
        Self {
            baseline: Some(turn_context_item.collaboration_mode.clone()),
            enabled: false,
        }
    }
}

impl WorldStateSection for CollaborationModeState {
    fn render_diff(&self, previous: &Self) -> Option<ResponseItem> {
        if !self.enabled || self.baseline == previous.baseline {
            return None;
        }
        CollaborationModeInstructions::from_collaboration_mode(self.baseline.as_ref()?.as_ref()?)
            .map(|instructions| developer_message(instructions.render()))
    }
}
