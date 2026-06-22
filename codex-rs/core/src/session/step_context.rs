use crate::environment_selection::TurnEnvironmentSnapshot;

/// Request-scoped state that may change between model sampling requests.
#[derive(Debug)]
pub(crate) struct StepContext {
    pub(crate) environments: TurnEnvironmentSnapshot,
}
