//! Resource identifier creation error.

use core::fmt;

#[cfg(feature = "std")]
use std::error;

use crate::validate::Error;

/// Error on conversion into an IRI type.
///
/// Enabled by `alloc` or `std` feature.
// This type itself does not require `alloc` or `std, but the type is used only when `alloc`
// feature is enabled. To avoid exporting unused stuff, the type (and the `types::generic::error`
// module) is available only when necessary.
//
// Note that all types which implement `Spec` also implement `SpecInternal`.
pub struct CreationError<T> {
    /// Soruce data.
    source: T,
    /// Validation error.
    error: Error,
}

impl<T> CreationError<T> {
    /// Returns the source data.
    #[must_use]
    pub fn into_source(self) -> T {
        self.source
    }

    /// Returns the validation error.
    #[must_use]
    pub fn validation_error(&self) -> Error {
        self.error
    }

    /// Creates a new `CreationError`.
    #[must_use]
    pub(crate) fn new(error: Error, source: T) -> Self {
        Self { source, error }
    }
}

impl<T: fmt::Debug> fmt::Debug for CreationError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CreationError")
            .field("source", &self.source)
            .field("error", &self.error)
            .finish()
    }
}

impl<T: Clone> Clone for CreationError<T> {
    fn clone(&self) -> Self {
        Self {
            source: self.source.clone(),
            error: self.error,
        }
    }
}

impl<T> fmt::Display for CreationError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error.fmt(f)
    }
}

#[cfg(feature = "std")]
impl<T: fmt::Debug> error::Error for CreationError<T> {}
