//! Errors related to URI templates.

use core::fmt;

#[cfg(feature = "std")]
use std::error;

/// Template construction and expansion error kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ErrorKind {
    /// Cannot write to the backend.
    WriteFailed,
    /// Expression is not closed.
    ExpressionNotClosed,
    /// Invalid character.
    InvalidCharacter,
    /// Invalid expression.
    InvalidExpression,
    /// Invalid percent-encoded triplets.
    InvalidPercentEncoding,
    /// Invalid UTF-8 bytes.
    InvalidUtf8,
    /// Unexpected value type for the variable.
    UnexpectedValueType,
    /// Unsupported operator, including operators reserved for future.
    UnsupportedOperator,
}

impl ErrorKind {
    /// Returns the error message.
    #[must_use]
    fn as_str(self) -> &'static str {
        match self {
            Self::WriteFailed => "failed to write to the backend writer",
            Self::ExpressionNotClosed => "expression not closed",
            Self::InvalidCharacter => "invalid character",
            Self::InvalidExpression => "invalid expression",
            Self::InvalidPercentEncoding => "invalid percent-encoded triplets",
            Self::InvalidUtf8 => "invalid utf-8 byte sequence",
            Self::UnexpectedValueType => "unexpected value type for the variable",
            Self::UnsupportedOperator => "unsupported operator",
        }
    }
}

/// Template construction and expansion error.
///
// Note that this type should implement `Copy` trait.
// To return additional non-`Copy` data as an error, use wrapper type
// (as `std::string::FromUtf8Error` contains `std::str::Utf8Error`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Error {
    /// Error kind.
    kind: ErrorKind,
    /// Location (byte position of the error).
    location: usize,
}

impl Error {
    /// Creates a new `Error`.
    ///
    /// For internal use.
    #[inline]
    #[must_use]
    pub(super) fn new(kind: ErrorKind, location: usize) -> Self {
        Self { kind, location }
    }

    /// Returns the byte position the error is detected.
    ///
    /// NOTE: This is not a part of the public API since the value to be
    /// returned (i.e., the definition of the "position" of an error) is not
    /// guaranteed to be stable.
    #[cfg(test)]
    pub(super) fn location(&self) -> usize {
        self.location
    }
}

impl fmt::Display for Error {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid URI template: {} (at {}-th byte)",
            self.kind.as_str(),
            self.location
        )
    }
}

#[cfg(feature = "std")]
impl error::Error for Error {}

/// Error on conversion into a URI template type.
// TODO: Unifiable to `types::CreationError`?
#[cfg(feature = "alloc")]
pub struct CreationError<T> {
    /// Soruce data.
    source: T,
    /// Validation error.
    error: Error,
}

#[cfg(feature = "alloc")]
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

#[cfg(feature = "alloc")]
impl<T: fmt::Debug> fmt::Debug for CreationError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CreationError")
            .field("source", &self.source)
            .field("error", &self.error)
            .finish()
    }
}

#[cfg(feature = "alloc")]
impl<T: Clone> Clone for CreationError<T> {
    fn clone(&self) -> Self {
        Self {
            source: self.source.clone(),
            error: self.error,
        }
    }
}

#[cfg(feature = "alloc")]
impl<T> fmt::Display for CreationError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error.fmt(f)
    }
}

#[cfg(feature = "std")]
impl<T: fmt::Debug> error::Error for CreationError<T> {}
