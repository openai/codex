use codex_protocol::ThreadId;

use crate::ExtensionData;

/// Host context available while extensions contribute turn-scoped context fragments.
#[derive(Clone, Copy)]
pub struct TurnContextContributionInput<'a> {
    /// Stable host-owned thread identifier.
    pub thread_id: ThreadId,
    /// Stable host-owned turn identifier.
    pub turn_id: &'a str,
    /// Store scoped to the host session runtime.
    pub session_store: &'a ExtensionData,
    /// Store scoped to this thread runtime.
    pub thread_store: &'a ExtensionData,
    /// Store scoped to this turn.
    pub turn_store: &'a ExtensionData,
    /// Store scoped to this sampling step.
    pub step_store: &'a ExtensionData,
    /// Effective model context window for this turn, when known.
    pub model_context_window: Option<i64>,
}
