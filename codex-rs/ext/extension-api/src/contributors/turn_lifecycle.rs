use codex_protocol::protocol::TurnAbortReason;

use crate::ExtensionData;

/// Input supplied when the host starts a turn.
pub struct TurnStartInput<'a> {
    /// Store scoped to the host session runtime.
    pub session_store: &'a ExtensionData,
    /// Store scoped to this thread runtime.
    pub thread_store: &'a ExtensionData,
    /// Store scoped to this turn runtime.
    pub turn_store: &'a ExtensionData,
}

/// Input supplied when the host completes a turn.
pub struct TurnStopInput<'a> {
    /// Store scoped to the host session runtime.
    pub session_store: &'a ExtensionData,
    /// Store scoped to this thread runtime.
    pub thread_store: &'a ExtensionData,
    /// Store scoped to this turn runtime.
    pub turn_store: &'a ExtensionData,
}

/// Input supplied when the host aborts a turn.
pub struct TurnAbortInput<'a> {
    /// Reason the host aborted the turn.
    pub reason: TurnAbortReason,
    /// Store scoped to the host session runtime.
    pub session_store: &'a ExtensionData,
    /// Store scoped to this thread runtime.
    pub thread_store: &'a ExtensionData,
    /// Store scoped to this turn runtime.
    pub turn_store: &'a ExtensionData,
}
