use std::fmt;

/// Possible errors of [`Pool::get()`] operation.
///
/// [`Pool::get()`]: super::Pool::get
#[derive(Clone, Copy, Debug)]
pub enum PoolError {
    /// Operation timeout happened.
    Timeout,

    /// Pool has been closed.
    Closed,

    /// No runtime specified.
    NoRuntimeSpecified,
}

impl fmt::Display for PoolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Timeout => write!(
                f,
                "Timeout occurred while waiting for an object to become available",
            ),
            Self::Closed => write!(f, "Pool has been closed"),
            Self::NoRuntimeSpecified => write!(f, "No runtime specified"),
        }
    }
}

impl std::error::Error for PoolError {}
