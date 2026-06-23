use crate::environment_selection::TurnEnvironmentSnapshot;
use crate::session::turn_context::TurnContext;

/// Request-scoped state that may change between model sampling requests.
#[derive(Debug)]
pub(crate) struct StepContext {
    pub(crate) environments: TurnEnvironmentSnapshot,
}

impl StepContext {
    pub(crate) fn from_turn_context(turn_context: &TurnContext) -> Self {
        Self {
            environments: turn_context.environments.clone(),
        }
    }
}
