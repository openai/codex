//! Definitions used in derived implementations of [`core::ops`] traits.

use core::fmt;

/// Error returned by the derived implementations when an arithmetic or logic
/// operation is invoked on a unit-like variant of an enum.
#[derive(Clone, Copy, Debug)]
pub struct UnitError {
    operation_name: &'static str,
}

impl UnitError {
    #[doc(hidden)]
    #[must_use]
    #[inline]
    pub const fn new(operation_name: &'static str) -> Self {
        Self { operation_name }
    }
}

impl fmt::Display for UnitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Cannot {}() unit variants", self.operation_name)
    }
}

#[cfg(feature = "std")]
impl std::error::Error for UnitError {}
