//! Error that occurred at some stage of parsing

use core::convert::Infallible;
use core::fmt;

use crate::error::{self, ParseFromDescription, TryFromParsed};

/// An error that occurred at some stage of parsing.
#[non_exhaustive]
#[allow(variant_size_differences, reason = "only triggers on some platforms")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Parse {
    #[expect(missing_docs)]
    TryFromParsed(TryFromParsed),
    #[expect(missing_docs)]
    ParseFromDescription(ParseFromDescription),
    #[expect(missing_docs)]
    #[non_exhaustive]
    #[deprecated(
        since = "0.3.28",
        note = "no longer output. moved to the `ParseFromDescription` variant"
    )]
    UnexpectedTrailingCharacters {
        #[doc(hidden)]
        never: Infallible,
    },
}

impl fmt::Display for Parse {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TryFromParsed(err) => err.fmt(f),
            Self::ParseFromDescription(err) => err.fmt(f),
            #[allow(deprecated)]
            Self::UnexpectedTrailingCharacters { never } => match *never {},
        }
    }
}

impl core::error::Error for Parse {
    #[inline]
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::TryFromParsed(err) => Some(err),
            Self::ParseFromDescription(err) => Some(err),
            #[allow(deprecated)]
            Self::UnexpectedTrailingCharacters { never } => match *never {},
        }
    }
}

impl From<TryFromParsed> for Parse {
    #[inline]
    fn from(err: TryFromParsed) -> Self {
        Self::TryFromParsed(err)
    }
}

impl TryFrom<Parse> for TryFromParsed {
    type Error = error::DifferentVariant;

    #[inline]
    fn try_from(err: Parse) -> Result<Self, Self::Error> {
        match err {
            Parse::TryFromParsed(err) => Ok(err),
            _ => Err(error::DifferentVariant),
        }
    }
}

impl From<ParseFromDescription> for Parse {
    #[inline]
    fn from(err: ParseFromDescription) -> Self {
        Self::ParseFromDescription(err)
    }
}

impl TryFrom<Parse> for ParseFromDescription {
    type Error = error::DifferentVariant;

    #[inline]
    fn try_from(err: Parse) -> Result<Self, Self::Error> {
        match err {
            Parse::ParseFromDescription(err) => Ok(err),
            _ => Err(error::DifferentVariant),
        }
    }
}

impl From<Parse> for crate::Error {
    #[inline]
    fn from(err: Parse) -> Self {
        match err {
            Parse::TryFromParsed(err) => Self::TryFromParsed(err),
            Parse::ParseFromDescription(err) => Self::ParseFromDescription(err),
            #[allow(deprecated)]
            Parse::UnexpectedTrailingCharacters { never } => match never {},
        }
    }
}

impl TryFrom<crate::Error> for Parse {
    type Error = error::DifferentVariant;

    #[inline]
    fn try_from(err: crate::Error) -> Result<Self, Self::Error> {
        match err {
            crate::Error::ParseFromDescription(err) => Ok(Self::ParseFromDescription(err)),
            #[allow(deprecated)]
            crate::Error::UnexpectedTrailingCharacters { never } => match never {},
            crate::Error::TryFromParsed(err) => Ok(Self::TryFromParsed(err)),
            _ => Err(error::DifferentVariant),
        }
    }
}
