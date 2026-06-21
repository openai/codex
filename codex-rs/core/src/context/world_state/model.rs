use super::WorldStateSection;
use crate::context::ContextualUserFragment;
use crate::context::ModelSwitchInstructions;
use crate::session::turn_context::TurnContext;

#[derive(Debug)]
pub(crate) struct ModelState {
    model: String,
    instructions: String,
}

impl ModelState {
    pub(crate) fn from_turn_context(turn_context: &TurnContext) -> Self {
        Self {
            model: turn_context.model_info.slug.clone(),
            instructions: turn_context
                .model_info
                .get_model_instructions(turn_context.personality),
        }
    }

    pub(crate) fn from_previous_model(model: &str) -> Self {
        Self {
            model: model.to_string(),
            instructions: String::new(),
        }
    }
}

impl WorldStateSection for ModelState {
    fn render_diff(&self, previous: Option<&Self>) -> Option<Box<dyn ContextualUserFragment>> {
        let previous = previous?;
        if self.model == previous.model || self.instructions.is_empty() {
            return None;
        }
        Some(Box::new(ModelSwitchInstructions::new(&self.instructions)))
    }
}
