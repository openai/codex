//! Error parsing an input into a [`Parsed`](crate::parsing::Parsed) struct

use core::fmt;

use crate::error;

/// An error that occurred while parsing the input into a [`Parsed`](crate::parsing::Parsed) struct.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseFromDescription {
    /// A string literal was not what was expected.
    #[non_exhaustive]
    InvalidLiteral,
    /// A dynamic component was not valid.
    InvalidComponent(&'static str),
    /// The input was expected to have ended, but there are characters that remain.
    #[non_exhaustive]
    UnexpectedTrailingCharacters,
}

impl fmt::Display for ParseFromDescription {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLiteral => f.write_str("a character literal was not valid"),
            Self::InvalidComponent(name) => {
                write!(f, "the '{name}' component could not be parsed")
            }
            Self::UnexpectedTrailingCharacters => {
                f.write_str("unexpected trailing characters; the end of input was expected")
            }
        }
    }
}

impl core::error::Error for ParseFromDescription {}

impl From<ParseFromDescription> for crate::Error {
    #[inline]
    fn from(original: ParseFromDescription) -> Self {
        Self::ParseFromDescription(original)
    }
}

impl TryFrom<crate::Error> for ParseFromDescription {
    type Error = error::DifferentVariant;

    #[inline]
    fn try_from(err: crate::Error) -> Result<Self, Self::Error> {
        match err {
            crate::Error::ParseFromDescription(err) => Ok(err),
            _ => Err(error::DifferentVariant),
        }
    }
}
