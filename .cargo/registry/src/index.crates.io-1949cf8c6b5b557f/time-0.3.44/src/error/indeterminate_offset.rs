//! Indeterminate offset

use core::fmt;

use crate::error;

/// The system's UTC offset could not be determined at the given datetime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IndeterminateOffset;

impl fmt::Display for IndeterminateOffset {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("The system's UTC offset could not be determined")
    }
}

impl core::error::Error for IndeterminateOffset {}

impl From<IndeterminateOffset> for crate::Error {
    #[inline]
    fn from(err: IndeterminateOffset) -> Self {
        Self::IndeterminateOffset(err)
    }
}

impl TryFrom<crate::Error> for IndeterminateOffset {
    type Error = error::DifferentVariant;

    #[inline]
    fn try_from(err: crate::Error) -> Result<Self, Self::Error> {
        match err {
            crate::Error::IndeterminateOffset(err) => Ok(err),
            _ => Err(error::DifferentVariant),
        }
    }
}
