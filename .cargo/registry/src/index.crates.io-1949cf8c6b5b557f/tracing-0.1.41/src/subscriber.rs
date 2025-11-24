//! Collects and records trace data.
pub use tracing_core::subscriber::*;

#[cfg(feature = "std")]
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
pub use tracing_core::dispatcher::DefaultGuard;

/// Sets this [`Subscriber`] as the default for the current thread for the
/// duration of a closure.
///
/// The default subscriber is used when creating a new [`Span`] or
/// [`Event`].
///
///
/// [`Span`]: super::span::Span
/// [`Subscriber`]: super::subscriber::Subscriber
/// [`Event`]: super::event::Event
#[cfg(feature = "std")]
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
pub fn with_default<T, S>(subscriber: S, f: impl FnOnce() -> T) -> T
where
    S: Subscriber + Send + Sync + 'static,
{
    crate::dispatcher::with_default(&crate::Dispatch::new(subscriber), f)
}

/// Sets this subscriber as the global default for the duration of the entire program.
/// Will be used as a fallback if no thread-local subscriber has been set in a thread (using `with_default`.)
///
/// Can only be set once; subsequent attempts to set the global default will fail.
/// Returns whether the initialization was successful.
///
/// Note: Libraries should *NOT* call `set_global_default()`! That will cause conflicts when
/// executables try to set them later.
///
/// [span]: super::span
/// [`Subscriber`]: super::subscriber::Subscriber
/// [`Event`]: super::event::Event
pub fn set_global_default<S>(subscriber: S) -> Result<(), SetGlobalDefaultError>
where
    S: Subscriber + Send + Sync + 'static,
{
    crate::dispatcher::set_global_default(crate::Dispatch::new(subscriber))
}

/// Sets the [`Subscriber`] as the default for the current thread for the
/// duration of the lifetime of the returned [`DefaultGuard`].
///
/// The default subscriber is used when creating a new [`Span`] or [`Event`].
///
/// [`Span`]: super::span::Span
/// [`Subscriber`]: super::subscriber::Subscriber
/// [`Event`]: super::event::Event
/// [`DefaultGuard`]: super::dispatcher::DefaultGuard
#[cfg(feature = "std")]
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
#[must_use = "Dropping the guard unregisters the subscriber."]
pub fn set_default<S>(subscriber: S) -> DefaultGuard
where
    S: Subscriber + Send + Sync + 'static,
{
    crate::dispatcher::set_default(&crate::Dispatch::new(subscriber))
}

pub use tracing_core::dispatcher::SetGlobalDefaultError;
