use codex_protocol::ThreadId;
use codex_protocol::protocol::TurnAbortReason;

use crate::ExtensionData;

/// Input supplied when the host starts a turn.
pub struct TurnStartInput<'a> {
    /// Identifier for the thread containing this turn.
    pub thread_id: ThreadId,
    /// Identifier for the turn that is starting.
    pub turn_id: &'a str,
    /// Store scoped to the host session runtime.
    pub session_store: &'a ExtensionData,
    /// Store scoped to this thread runtime.
    pub thread_store: &'a ExtensionData,
    /// Store scoped to this turn runtime.
    pub turn_store: &'a ExtensionData,
}

/// Input supplied when the host completes a turn.
pub struct TurnStopInput<'a> {
    /// Identifier for the thread containing this turn.
    pub thread_id: ThreadId,
    /// Identifier for the turn that is stopping.
    pub turn_id: &'a str,
    /// Store scoped to the host session runtime.
    pub session_store: &'a ExtensionData,
    /// Store scoped to this thread runtime.
    pub thread_store: &'a ExtensionData,
    /// Store scoped to this turn runtime.
    pub turn_store: &'a ExtensionData,
}

/// Input supplied when the host aborts a turn.
pub struct TurnAbortInput<'a> {
    /// Identifier for the thread containing this turn.
    pub thread_id: ThreadId,
    /// Identifier for the turn that is aborting.
    pub turn_id: &'a str,
    /// Reason the host aborted the turn.
    pub reason: TurnAbortReason,
    /// Store scoped to the host session runtime.
    pub session_store: &'a ExtensionData,
    /// Store scoped to this thread runtime.
    pub thread_store: &'a ExtensionData,
    /// Store scoped to this turn runtime.
    pub turn_store: &'a ExtensionData,
}
