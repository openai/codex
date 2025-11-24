//! Conversion range error

use core::fmt;

use crate::error;

/// An error type indicating that a conversion failed because the target type could not store the
/// initial value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConversionRange;

impl fmt::Display for ConversionRange {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Source value is out of range for the target type")
    }
}

impl core::error::Error for ConversionRange {}

impl From<ConversionRange> for crate::Error {
    #[inline]
    fn from(err: ConversionRange) -> Self {
        Self::ConversionRange(err)
    }
}

impl TryFrom<crate::Error> for ConversionRange {
    type Error = error::DifferentVariant;

    #[inline]
    fn try_from(err: crate::Error) -> Result<Self, Self::Error> {
        match err {
            crate::Error::ConversionRange(err) => Ok(err),
            _ => Err(error::DifferentVariant),
        }
    }
}
