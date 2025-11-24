//! Error formatting a struct

use alloc::boxed::Box;
use core::fmt;
use std::io;

use crate::error;

/// An error occurred when formatting.
#[non_exhaustive]
#[derive(Debug)]
pub enum Format {
    /// The type being formatted does not contain sufficient information to format a component.
    #[non_exhaustive]
    InsufficientTypeInformation,
    /// The component named has a value that cannot be formatted into the requested format.
    ///
    /// This variant is only returned when using well-known formats.
    InvalidComponent(&'static str),
    /// A component provided was out of range.
    ComponentRange(Box<error::ComponentRange>),
    /// A value of `std::io::Error` was returned internally.
    StdIo(io::Error),
}

impl fmt::Display for Format {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InsufficientTypeInformation => f.write_str(
                "The type being formatted does not contain sufficient information to format a \
                 component.",
            ),
            Self::InvalidComponent(component) => write!(
                f,
                "The {component} component cannot be formatted into the requested format."
            ),
            Self::ComponentRange(err) => err.fmt(f),
            Self::StdIo(err) => err.fmt(f),
        }
    }
}

impl From<error::ComponentRange> for Format {
    #[inline]
    fn from(err: error::ComponentRange) -> Self {
        Self::ComponentRange(Box::new(err))
    }
}

impl From<io::Error> for Format {
    #[inline]
    fn from(err: io::Error) -> Self {
        Self::StdIo(err)
    }
}

impl TryFrom<Format> for error::ComponentRange {
    type Error = error::DifferentVariant;

    #[inline]
    fn try_from(err: Format) -> Result<Self, Self::Error> {
        match err {
            Format::ComponentRange(err) => Ok(*err),
            _ => Err(error::DifferentVariant),
        }
    }
}

impl TryFrom<Format> for io::Error {
    type Error = error::DifferentVariant;

    #[inline]
    fn try_from(err: Format) -> Result<Self, Self::Error> {
        match err {
            Format::StdIo(err) => Ok(err),
            _ => Err(error::DifferentVariant),
        }
    }
}

impl core::error::Error for Format {
    #[inline]
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::InsufficientTypeInformation | Self::InvalidComponent(_) => None,
            Self::ComponentRange(err) => Some(&**err),
            Self::StdIo(err) => Some(err),
        }
    }
}

impl From<Format> for crate::Error {
    #[inline]
    fn from(original: Format) -> Self {
        Self::Format(original)
    }
}

impl TryFrom<crate::Error> for Format {
    type Error = error::DifferentVariant;

    #[inline]
    fn try_from(err: crate::Error) -> Result<Self, Self::Error> {
        match err {
            crate::Error::Format(err) => Ok(err),
            _ => Err(error::DifferentVariant),
        }
    }
}

#[cfg(feature = "serde")]
impl Format {
    /// Obtain an error type for the serializer.
    #[doc(hidden)] // Exposed only for the `declare_format_string` macro
    #[inline]
    pub fn into_invalid_serde_value<S: serde::Serializer>(self) -> S::Error {
        use serde::ser::Error;
        S::Error::custom(self)
    }
}
