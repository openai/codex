use super::WorldStateSection;
use super::developer_message;
use crate::context::ContextualUserFragment;
use crate::context::RealtimeEndInstructions;
use crate::context::RealtimeStartInstructions;
use crate::context::RealtimeStartWithInstructions;
use crate::session::turn_context::TurnContext;
use codex_protocol::models::ResponseItem;

#[derive(Debug, Default)]
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

    pub(crate) fn rendered_diff(&self, previous: &Self) -> Option<String> {
        match (previous.active, self.active.unwrap_or(false)) {
            (Some(true), false) => Some(RealtimeEndInstructions::new("inactive").render()),
            (Some(false), true) | (None, true) => Some(
                if let Some(instructions) = self.start_instructions.as_deref() {
                    RealtimeStartWithInstructions::new(instructions).render()
                } else {
                    RealtimeStartInstructions.render()
                },
            ),
            (Some(true), true) | (Some(false), false) => None,
            (None, false) => previous
                .active_fallback
                .filter(|active| *active)
                .map(|_| RealtimeEndInstructions::new("inactive").render()),
        }
    }
}

impl WorldStateSection for RealtimeState {
    fn render_diff(&self, previous: &Self) -> Option<ResponseItem> {
        self.rendered_diff(previous).map(developer_message)
    }
}
