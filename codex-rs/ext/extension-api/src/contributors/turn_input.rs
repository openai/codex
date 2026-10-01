use codex_file_system::EnvironmentAccess;
use codex_protocol::user_input::UserInput;
use codex_utils_path_uri::PathUri;
use std::fmt;

/// Host-owned turn environment summary visible to turn-input contributors.
#[derive(Clone)]
pub struct TurnInputEnvironment<'a> {
    /// Stable host environment id used to route executor-scoped capabilities.
    pub environment_id: String,
    /// Effective working directory for this turn in the environment.
    pub cwd: PathUri,
    /// Whether this is the primary environment for the turn.
    pub is_primary: bool,
    /// Filesystem access with the permissions captured for this model turn.
    pub fs: &'a dyn EnvironmentAccess,
}

/// Turn facts supplied before the host records turn-local model input items.
#[derive(Clone)]
pub struct TurnInputContext<'a> {
    /// Stable host-owned turn identifier.
    pub turn_id: String,
    /// User input submitted for this turn.
    pub user_input: Vec<UserInput>,
    /// Resolved turn environments, in host priority order.
    pub environments: Vec<TurnInputEnvironment<'a>>,
}

impl fmt::Debug for TurnInputEnvironment<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TurnInputEnvironment")
            .field("environment_id", &self.environment_id)
            .field("cwd", &self.cwd)
            .field("is_primary", &self.is_primary)
            .finish_non_exhaustive()
    }
}

impl fmt::Debug for TurnInputContext<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TurnInputContext")
            .field("turn_id", &self.turn_id)
            .field("user_input", &self.user_input)
            .field("environments", &self.environments)
            .finish_non_exhaustive()
    }
}
