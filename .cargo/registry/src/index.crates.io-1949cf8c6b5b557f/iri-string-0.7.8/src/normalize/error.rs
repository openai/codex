//! Normalization and resolution error.

use core::fmt;

/// IRI normalization and resolution error.
///
/// For detail about resolution failure, see [the module documentation][`crate::resolve`].
#[derive(Debug, Clone)]
pub struct Error(());

impl Error {
    /// Creates a new error.
    pub(crate) fn new() -> Self {
        Self(())
    }
}

impl fmt::Display for Error {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("unresolvable IRI")
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Error {}
