use std::future::Future;
use std::pin::Pin;

use codex_protocol::config_types::CollaborationMode;
use codex_protocol::protocol::InitialGoal;

use crate::ExtensionData;

/// Input supplied before the host commits settings or starts an initial goal turn.
pub struct InitialGoalInput<'a> {
    /// Stable host-owned turn identifier.
    pub turn_id: &'a str,
    /// Goal objective requested for this turn.
    pub goal: &'a InitialGoal,
    /// Effective collaboration mode prepared for this turn.
    pub collaboration_mode: &'a CollaborationMode,
    /// Store scoped to the host session runtime.
    pub session_store: &'a ExtensionData,
    /// Store scoped to this thread runtime.
    pub thread_store: &'a ExtensionData,
}

/// Error returned while preparing an initial goal turn.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InitialGoalError {
    /// The request is invalid and should be reported to the caller.
    InvalidRequest(String),
    /// Goal persistence or runtime preparation failed internally.
    Internal(String),
}

/// Extension contribution that atomically replaces a goal before a turn starts.
pub trait InitialGoalContributor: Send + Sync {
    /// Persist and prepare the requested goal before the host commits the turn.
    fn replace_for_turn<'a>(
        &'a self,
        input: InitialGoalInput<'a>,
    ) -> Pin<Box<dyn Future<Output = Result<(), InitialGoalError>> + Send + 'a>>;
}
