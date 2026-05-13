use codex_protocol::ThreadId;

use crate::ExtensionData;

/// Input supplied when the host starts a runtime for a thread.
pub struct ThreadStartInput<'a, C> {
    /// Identifier for the thread whose runtime is starting.
    pub thread_id: ThreadId,
    /// Host configuration visible at thread start.
    pub config: &'a C,
    /// Store scoped to the host session runtime.
    pub session_store: &'a ExtensionData,
    /// Store scoped to this thread runtime.
    pub thread_store: &'a ExtensionData,
}

/// Input supplied when the host resumes an existing thread.
pub struct ThreadResumeInput<'a> {
    /// Identifier for the thread being resumed.
    pub thread_id: ThreadId,
    /// Store scoped to the host session runtime.
    pub session_store: &'a ExtensionData,
    /// Store scoped to this thread runtime.
    pub thread_store: &'a ExtensionData,
}

/// Input supplied when the host stops a thread runtime.
pub struct ThreadStopInput<'a> {
    /// Identifier for the thread whose runtime is stopping.
    pub thread_id: ThreadId,
    /// Store scoped to the host session runtime.
    pub session_store: &'a ExtensionData,
    /// Store scoped to this thread runtime.
    pub thread_store: &'a ExtensionData,
}
