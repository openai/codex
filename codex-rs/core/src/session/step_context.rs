use std::sync::Arc;

use crate::environment_selection::TurnEnvironmentSnapshot;
use crate::session::turn_context::TurnContext;

/// Request-scoped state that may change between model sampling requests.
#[derive(Debug)]
pub(crate) struct StepContext {
    pub(crate) turn: Arc<TurnContext>,
    pub(crate) environments: TurnEnvironmentSnapshot,
}

impl StepContext {
    pub(crate) fn new(turn: Arc<TurnContext>, environments: TurnEnvironmentSnapshot) -> Self {
        Self { turn, environments }
    }
}
