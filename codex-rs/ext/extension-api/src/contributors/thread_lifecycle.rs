use std::future::Future;
use std::pin::Pin;

use codex_protocol::ThreadId;

use crate::ExtensionData;

/// Future returned by one async thread-lifecycle contribution.
pub type ThreadLifecycleFuture<'a> = Pin<Box<dyn Future<Output = Result<(), String>> + Send + 'a>>;

/// Input supplied when the host starts a runtime for a thread.
pub struct ThreadStartInput<'a, C> {
    /// Identifier for the thread whose runtime is starting.
    pub thread_id: ThreadId,
    /// Host configuration visible at thread start.
    pub config: &'a C,
    /// Store shared by all threads in the same session.
    pub session_store: &'a ExtensionData,
    /// Store scoped to this thread runtime.
    pub thread_store: &'a ExtensionData,
}

/// Input supplied when the host resumes an existing thread.
pub struct ThreadResumeInput<'a> {
    /// Identifier for the thread being resumed.
    pub thread_id: ThreadId,
    /// Store shared by all threads in the same session.
    pub session_store: &'a ExtensionData,
    /// Store scoped to this thread runtime.
    pub thread_store: &'a ExtensionData,
}

/// Input supplied when the host stops a thread runtime.
pub struct ThreadStopInput<'a> {
    /// Identifier for the thread whose runtime is stopping.
    pub thread_id: ThreadId,
    /// Store shared by all threads in the same session.
    pub session_store: &'a ExtensionData,
    /// Store scoped to this thread runtime.
    pub thread_store: &'a ExtensionData,
}

/// Contributor for host-owned thread lifecycle gates.
///
/// Implementations should use these callbacks to seed, rehydrate, or flush
/// extension-private thread state. Heavy dependencies belong on the extension
/// value created by the host, not in these inputs.
pub trait ThreadLifecycleContributor<C>: Send + Sync {
    /// Called after thread-scoped extension stores are created, before later
    /// contributors can read from them.
    fn on_thread_start(&self, _input: ThreadStartInput<'_, C>) {}

    /// Called after the host has resumed an existing thread. This can happen
    /// either after constructing a runtime from persisted history or after
    /// reattaching to an already loaded runtime.
    fn on_thread_resume<'a>(&'a self, _input: ThreadResumeInput<'a>) -> ThreadLifecycleFuture<'a> {
        Box::pin(async { Ok(()) })
    }

    /// Called before the host drops the thread runtime and thread-scoped store.
    fn on_thread_stop<'a>(&'a self, _input: ThreadStopInput<'a>) -> ThreadLifecycleFuture<'a> {
        Box::pin(async { Ok(()) })
    }
}
