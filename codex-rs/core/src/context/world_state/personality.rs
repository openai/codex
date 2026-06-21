use super::WorldStateSection;
use super::developer_message;
use crate::context::ContextualUserFragment;
use crate::context::PersonalitySpecInstructions;
use crate::session::turn_context::TurnContext;
use codex_protocol::config_types::Personality;
use codex_protocol::models::ResponseItem;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::protocol::TurnContextItem;

#[derive(Debug, Default)]
pub(crate) struct PersonalityState {
    enabled: bool,
    model: Option<String>,
    personality: Option<Personality>,
    spec: Option<String>,
}

impl PersonalityState {
    pub(crate) fn from_turn_context(turn_context: &TurnContext, enabled: bool) -> Self {
        let personality = turn_context.personality;
        Self {
            enabled,
            model: Some(turn_context.model_info.slug.clone()),
            personality,
            spec: personality
                .and_then(|personality| Self::message(&turn_context.model_info, personality)),
        }
    }

    pub(crate) fn from_turn_context_item(turn_context_item: &TurnContextItem) -> Self {
        Self {
            enabled: false,
            model: Some(turn_context_item.model.clone()),
            personality: turn_context_item.personality,
            spec: None,
        }
    }

    pub(crate) fn message(model_info: &ModelInfo, personality: Personality) -> Option<String> {
        model_info
            .model_messages
            .as_ref()
            .and_then(|spec| spec.get_personality_message(Some(personality)))
            .filter(|message| !message.is_empty())
    }
}

impl WorldStateSection for PersonalityState {
    fn render_diff(&self, previous: &Self) -> Option<ResponseItem> {
        if !self.enabled || self.model != previous.model || self.personality == previous.personality
        {
            return None;
        }
        self.personality?;
        self.spec
            .as_ref()
            .map(|spec| developer_message(PersonalitySpecInstructions::new(spec).render()))
    }
}
