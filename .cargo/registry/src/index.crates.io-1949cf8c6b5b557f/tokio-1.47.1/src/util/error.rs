// Some combinations of features may not use these constants.
#![cfg_attr(not(feature = "full"), allow(dead_code))]

/// Error string explaining that the Tokio context hasn't been instantiated.
pub(crate) const CONTEXT_MISSING_ERROR: &str =
    "there is no reactor running, must be called from the context of a Tokio 1.x runtime";

/// Error string explaining that the Tokio context is shutting down and cannot drive timers.
pub(crate) const RUNTIME_SHUTTING_DOWN_ERROR: &str =
    "A Tokio 1.x context was found, but it is being shutdown.";

/// Error string explaining that the Tokio context is not available because the
/// thread-local storing it has been destroyed. This usually only happens during
/// destructors of other thread-locals.
pub(crate) const THREAD_LOCAL_DESTROYED_ERROR: &str =
    "The Tokio context thread-local variable has been destroyed.";
