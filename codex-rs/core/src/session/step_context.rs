use std::sync::Arc;

use crate::agents_md::LoadedAgentsMd;
use crate::environment_selection::TurnEnvironmentSnapshot;
use crate::session::turn_context::TurnContext;

/// Request-scoped state that may change between model sampling requests.
#[derive(Debug)]
pub(crate) struct StepContext {
    pub(crate) turn: Arc<TurnContext>,
    pub(crate) environments: TurnEnvironmentSnapshot,
    /// The canonical AGENTS.md value observed with this environment snapshot.
    pub(crate) loaded_agents_md: Option<Arc<LoadedAgentsMd>>,
}

impl StepContext {
    pub(crate) fn new(
        turn: Arc<TurnContext>,
        environments: TurnEnvironmentSnapshot,
        loaded_agents_md: Option<Arc<LoadedAgentsMd>>,
    ) -> Self {
        Self {
            turn,
            environments,
            loaded_agents_md,
        }
    }
}
