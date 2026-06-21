use super::WorldStateSection;
use crate::context::ContextualUserFragment;
use crate::context::MultiAgentModeInstructions;
use crate::session::multi_agents;
use crate::session::turn_context::TurnContext;
use codex_features::Feature;
use codex_protocol::config_types::MultiAgentMode;
use codex_protocol::protocol::TurnContextItem;

#[derive(Debug)]
pub(crate) struct MultiAgentModeState(Option<MultiAgentMode>);

impl MultiAgentModeState {
    pub(crate) fn from_turn_context(turn_context: &TurnContext) -> Self {
        Self(multi_agents::effective_multi_agent_mode(
            turn_context.multi_agent_version,
            &turn_context.config.multi_agent_v2,
            &turn_context.session_source,
            turn_context.multi_agent_mode,
            turn_context
                .config
                .features
                .enabled(Feature::MultiAgentMode),
        ))
    }

    pub(crate) fn from_turn_context_item(turn_context_item: &TurnContextItem) -> Self {
        Self(turn_context_item.multi_agent_mode)
    }
}

impl WorldStateSection for MultiAgentModeState {
    fn render_diff(&self, previous: Option<&Self>) -> Option<Box<dyn ContextualUserFragment>> {
        if previous.is_some_and(|previous| self.0 == previous.0) {
            return None;
        }
        let mode = match self.0 {
            Some(mode) => mode,
            None if previous
                .is_some_and(|previous| previous.0 == Some(MultiAgentMode::Proactive)) =>
            {
                MultiAgentMode::ExplicitRequestOnly
            }
            None => return None,
        };
        Some(Box::new(MultiAgentModeInstructions::new(mode)))
    }
}
