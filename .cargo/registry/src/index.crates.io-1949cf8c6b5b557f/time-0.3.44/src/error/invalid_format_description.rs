//! Invalid format description

use alloc::string::String;
use core::fmt;

use crate::error;

/// The format description provided was not valid.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvalidFormatDescription {
    /// There was a bracket pair that was opened but not closed.
    #[non_exhaustive]
    UnclosedOpeningBracket {
        /// The zero-based index of the opening bracket.
        index: usize,
    },
    /// A component name is not valid.
    #[non_exhaustive]
    InvalidComponentName {
        /// The name of the invalid component name.
        name: String,
        /// The zero-based index the component name starts at.
        index: usize,
    },
    /// A modifier is not valid.
    #[non_exhaustive]
    InvalidModifier {
        /// The value of the invalid modifier.
        value: String,
        /// The zero-based index the modifier starts at.
        index: usize,
    },
    /// A component name is missing.
    #[non_exhaustive]
    MissingComponentName {
        /// The zero-based index where the component name should start.
        index: usize,
    },
    /// A required modifier is missing.
    #[non_exhaustive]
    MissingRequiredModifier {
        /// The name of the modifier that is missing.
        name: &'static str,
        /// The zero-based index of the component.
        index: usize,
    },
    /// Something was expected, but not found.
    #[non_exhaustive]
    Expected {
        /// What was expected to be present, but wasn't.
        what: &'static str,
        /// The zero-based index the item was expected to be found at.
        index: usize,
    },
    /// Certain behavior is not supported in the given context.
    #[non_exhaustive]
    NotSupported {
        /// The behavior that is not supported.
        what: &'static str,
        /// The context in which the behavior is not supported.
        context: &'static str,
        /// The zero-based index the error occurred at.
        index: usize,
    },
}

impl From<InvalidFormatDescription> for crate::Error {
    #[inline]
    fn from(original: InvalidFormatDescription) -> Self {
        Self::InvalidFormatDescription(original)
    }
}

impl TryFrom<crate::Error> for InvalidFormatDescription {
    type Error = error::DifferentVariant;

    #[inline]
    fn try_from(err: crate::Error) -> Result<Self, Self::Error> {
        match err {
            crate::Error::InvalidFormatDescription(err) => Ok(err),
            _ => Err(error::DifferentVariant),
        }
    }
}

impl fmt::Display for InvalidFormatDescription {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use InvalidFormatDescription::*;
        match self {
            UnclosedOpeningBracket { index } => {
                write!(f, "unclosed opening bracket at byte index {index}")
            }
            InvalidComponentName { name, index } => {
                write!(f, "invalid component name `{name}` at byte index {index}")
            }
            InvalidModifier { value, index } => {
                write!(f, "invalid modifier `{value}` at byte index {index}")
            }
            MissingComponentName { index } => {
                write!(f, "missing component name at byte index {index}")
            }
            MissingRequiredModifier { name, index } => {
                write!(
                    f,
                    "missing required modifier `{name}` for component at byte index {index}"
                )
            }
            Expected {
                what: expected,
                index,
            } => {
                write!(f, "expected {expected} at byte index {index}")
            }
            NotSupported {
                what,
                context,
                index,
            } => {
                if context.is_empty() {
                    write!(f, "{what} is not supported at byte index {index}")
                } else {
                    write!(
                        f,
                        "{what} is not supported in {context} at byte index {index}"
                    )
                }
            }
        }
    }
}

impl core::error::Error for InvalidFormatDescription {}
