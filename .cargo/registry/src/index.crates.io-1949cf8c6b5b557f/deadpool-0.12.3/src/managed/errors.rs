use std::{borrow::Cow, fmt};

use super::hooks::HookError;

/// Possible errors returned by the [`Manager::recycle()`] method.
///
/// [`Manager::recycle()`]: super::Manager::recycle
#[derive(Debug)]
pub enum RecycleError<E> {
    /// Recycling failed for some other reason.
    Message(Cow<'static, str>),

    /// Error caused by the backend.
    Backend(E),
}

impl<E> RecycleError<E> {
    /// Convenience constructor function for the `HookError::Message`
    /// variant.
    pub fn message(msg: impl Into<Cow<'static, str>>) -> Self {
        Self::Message(msg.into())
    }
}

impl<E> From<E> for RecycleError<E> {
    fn from(e: E) -> Self {
        Self::Backend(e)
    }
}

impl<E: fmt::Display> fmt::Display for RecycleError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Message(msg) => write!(f, "Error occurred while recycling an object: {}", msg),
            Self::Backend(e) => write!(f, "Error occurred while recycling an object: {}", e),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for RecycleError<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Message(_) => None,
            Self::Backend(e) => Some(e),
        }
    }
}

/// Possible steps causing the timeout in an error returned by [`Pool::get()`]
/// method.
///
/// [`Pool::get()`]: super::Pool::get
#[derive(Clone, Copy, Debug)]
pub enum TimeoutType {
    /// Timeout happened while waiting for a slot to become available.
    Wait,

    /// Timeout happened while creating a new object.
    Create,

    /// Timeout happened while recycling an object.
    Recycle,
}

/// Possible errors returned by [`Pool::get()`] method.
///
/// [`Pool::get()`]: super::Pool::get
#[derive(Debug)]
pub enum PoolError<E> {
    /// Timeout happened.
    Timeout(TimeoutType),

    /// Backend reported an error.
    Backend(E),

    /// [`Pool`] has been closed.
    ///
    /// [`Pool`]: super::Pool
    Closed,

    /// No [`Runtime`] was specified.
    ///
    /// [`Runtime`]: crate::Runtime
    NoRuntimeSpecified,

    /// A `post_create` hook reported an error.
    PostCreateHook(HookError<E>),
}

impl<E> From<E> for PoolError<E> {
    fn from(e: E) -> Self {
        Self::Backend(e)
    }
}

impl<E: fmt::Display> fmt::Display for PoolError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Timeout(tt) => match tt {
                TimeoutType::Wait => write!(
                    f,
                    "Timeout occurred while waiting for a slot to become available"
                ),
                TimeoutType::Create => write!(f, "Timeout occurred while creating a new object"),
                TimeoutType::Recycle => write!(f, "Timeout occurred while recycling an object"),
            },
            Self::Backend(e) => write!(f, "Error occurred while creating a new object: {}", e),
            Self::Closed => write!(f, "Pool has been closed"),
            Self::NoRuntimeSpecified => write!(f, "No runtime specified"),
            Self::PostCreateHook(e) => writeln!(f, "`post_create` hook failed: {}", e),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for PoolError<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Timeout(_) | Self::Closed | Self::NoRuntimeSpecified => None,
            Self::Backend(e) => Some(e),
            Self::PostCreateHook(e) => Some(e),
        }
    }
}
