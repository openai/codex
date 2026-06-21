use super::WorldStateSection;
use super::developer_message;
use crate::context::ContextualUserFragment;
use crate::context::MultiAgentModeInstructions;
use crate::session::multi_agents;
use crate::session::turn_context::TurnContext;
use codex_features::Feature;
use codex_protocol::config_types::MultiAgentMode;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::TurnContextItem;

#[derive(Debug, Default)]
pub(crate) struct MultiAgentModeState(Option<Option<MultiAgentMode>>);

impl MultiAgentModeState {
    pub(crate) fn from_turn_context(turn_context: &TurnContext) -> Self {
        Self(Some(multi_agents::effective_multi_agent_mode(
            turn_context.multi_agent_version,
            &turn_context.config.multi_agent_v2,
            &turn_context.session_source,
            turn_context.multi_agent_mode,
            turn_context
                .config
                .features
                .enabled(Feature::MultiAgentMode),
        )))
    }

    pub(crate) fn from_turn_context_item(turn_context_item: &TurnContextItem) -> Self {
        Self(Some(turn_context_item.multi_agent_mode))
    }
}

impl WorldStateSection for MultiAgentModeState {
    fn render_diff(&self, previous: &Self) -> Option<ResponseItem> {
        let previous_mode = previous.0.as_ref()?;
        let current_mode = self.0.as_ref()?;
        if current_mode == previous_mode {
            return None;
        }
        let mode = match current_mode {
            Some(mode) => *mode,
            None if *previous_mode == Some(MultiAgentMode::Proactive) => {
                MultiAgentMode::ExplicitRequestOnly
            }
            None => return None,
        };
        Some(developer_message(
            MultiAgentModeInstructions::new(mode).render(),
        ))
    }
}
