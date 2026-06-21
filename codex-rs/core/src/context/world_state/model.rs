use super::WorldStateSection;
use super::developer_message;
use crate::context::ContextualUserFragment;
use crate::context::ModelSwitchInstructions;
use crate::session::turn_context::TurnContext;
use codex_protocol::models::ResponseItem;

#[derive(Debug, Default)]
pub(crate) struct ModelState {
    model: Option<String>,
    instructions: String,
}

impl ModelState {
    pub(crate) fn from_turn_context(turn_context: &TurnContext) -> Self {
        Self {
            model: Some(turn_context.model_info.slug.clone()),
            instructions: turn_context
                .model_info
                .get_model_instructions(turn_context.personality),
        }
    }

    pub(crate) fn from_previous_model(model: Option<&str>) -> Self {
        Self {
            model: model.map(str::to_string),
            instructions: String::new(),
        }
    }

    pub(crate) fn rendered_diff(&self, previous: &Self) -> Option<String> {
        let previous_model = previous.model.as_ref()?;
        if self.model.as_ref() == Some(previous_model) || self.instructions.is_empty() {
            return None;
        }
        Some(ModelSwitchInstructions::new(&self.instructions).render())
    }
}

impl WorldStateSection for ModelState {
    fn render_diff(&self, previous: &Self) -> Option<ResponseItem> {
        self.rendered_diff(previous).map(developer_message)
    }
}
