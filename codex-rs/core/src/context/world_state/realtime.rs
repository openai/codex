use super::WorldStateSection;
use crate::context::ContextualUserFragment;
use crate::context::RealtimeEndInstructions;
use crate::context::RealtimeStartInstructions;
use crate::context::RealtimeStartWithInstructions;
use crate::session::turn_context::TurnContext;

#[derive(Debug)]
pub(crate) struct RealtimeState {
    active: Option<bool>,
    active_fallback: Option<bool>,
    start_instructions: Option<String>,
}

impl RealtimeState {
    pub(crate) fn from_turn_context(turn_context: &TurnContext) -> Self {
        Self {
            active: Some(turn_context.realtime_active),
            active_fallback: None,
            start_instructions: turn_context
                .config
                .experimental_realtime_start_instructions
                .clone(),
        }
    }

    pub(crate) fn from_previous(active: Option<bool>, active_fallback: Option<bool>) -> Self {
        Self {
            active,
            active_fallback,
            start_instructions: None,
        }
    }
}

impl WorldStateSection for RealtimeState {
    fn render_diff(&self, previous: Option<&Self>) -> Option<Box<dyn ContextualUserFragment>> {
        let previous_active = previous.and_then(|previous| previous.active);
        match (previous_active, self.active.unwrap_or(false)) {
            (Some(true), false) => Some(Box::new(RealtimeEndInstructions::new("inactive"))),
            (Some(false), true) | (None, true) => {
                if let Some(instructions) = self.start_instructions.as_deref() {
                    Some(Box::new(RealtimeStartWithInstructions::new(instructions)))
                } else {
                    Some(Box::new(RealtimeStartInstructions))
                }
            }
            (Some(true), true) | (Some(false), false) => None,
            (None, false) => previous
                .and_then(|previous| previous.active_fallback)
                .filter(|active| *active)
                .map(|_| {
                    Box::new(RealtimeEndInstructions::new("inactive"))
                        as Box<dyn ContextualUserFragment>
                }),
        }
    }
}
