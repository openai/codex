//! Different variant error

use core::fmt;

/// An error type indicating that a [`TryFrom`](core::convert::TryFrom) call failed because the
/// original value was of a different variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DifferentVariant;

impl fmt::Display for DifferentVariant {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "value was of a different variant than required")
    }
}

impl core::error::Error for DifferentVariant {}

impl From<DifferentVariant> for crate::Error {
    #[inline]
    fn from(err: DifferentVariant) -> Self {
        Self::DifferentVariant(err)
    }
}

impl TryFrom<crate::Error> for DifferentVariant {
    type Error = Self;

    #[inline]
    fn try_from(err: crate::Error) -> Result<Self, Self::Error> {
        match err {
            crate::Error::DifferentVariant(err) => Ok(err),
            _ => Err(Self),
        }
    }
}
