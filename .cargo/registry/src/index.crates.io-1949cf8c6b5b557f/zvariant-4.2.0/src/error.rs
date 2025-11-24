use serde::{de, ser};
use static_assertions::assert_impl_all;
use std::{convert::Infallible, error, fmt, io, result, sync::Arc};

/// Enum representing the max depth exceeded error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaxDepthExceeded {
    /// The maximum allowed depth for structures in encoding was exceeded.
    Structure,
    /// The maximum allowed depth for arrays in encoding was exceeded.
    Array,
    /// The maximum allowed depth for containers in encoding was exceeded.
    Container,
}

impl fmt::Display for MaxDepthExceeded {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Structure => write!(
                f,
                "Maximum allowed depth for structures in encoding was exceeded"
            ),
            Self::Array => write!(
                f,
                "Maximum allowed depth for arrays in encoding was exceeded"
            ),
            Self::Container => write!(
                f,
                "Maximum allowed depth for containers in encoding was exceeded"
            ),
        }
    }
}

/// Error type used by zvariant API.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// Generic error. All serde errors gets transformed into this variant.
    Message(String),

    /// Wrapper for [`std::io::Error`](https://doc.rust-lang.org/std/io/struct.Error.html)
    InputOutput(Arc<io::Error>),
    /// Type conversions errors.
    IncorrectType,
    /// Wrapper for [`std::str::Utf8Error`](https://doc.rust-lang.org/std/str/struct.Utf8Error.html)
    Utf8(std::str::Utf8Error),
    /// Non-0 padding byte(s) encountered.
    PaddingNot0(u8),
    /// The deserialized file descriptor is not in the given FD index.
    UnknownFd,
    /// Missing framing offset at the end of a GVariant-encoded container,
    MissingFramingOffset,
    /// The type (signature as first argument) being (de)serialized is not supported by the format.
    IncompatibleFormat(crate::Signature<'static>, crate::serialized::Format),
    /// The provided signature (first argument) was not valid for reading as the requested type.
    /// Details on the expected signatures are in the second argument.
    SignatureMismatch(crate::Signature<'static>, String),
    /// Out of bounds range specified.
    OutOfBounds,
    /// The maximum allowed depth for containers in encoding was exceeded.
    MaxDepthExceeded(MaxDepthExceeded),
}

assert_impl_all!(Error: Send, Sync, Unpin);

impl PartialEq for Error {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Error::Message(msg), Error::Message(other)) => msg == other,
            // Io is false
            (Error::IncorrectType, Error::IncorrectType) => true,
            (Error::Utf8(msg), Error::Utf8(other)) => msg == other,
            (Error::PaddingNot0(p), Error::PaddingNot0(other)) => p == other,
            (Error::UnknownFd, Error::UnknownFd) => true,
            (Error::MaxDepthExceeded(max1), Error::MaxDepthExceeded(max2)) => max1 == max2,
            (_, _) => false,
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Error::InputOutput(e) => Some(e),
            Error::Utf8(e) => Some(e),
            _ => None,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Message(s) => write!(f, "{s}"),
            Error::InputOutput(e) => e.fmt(f),
            Error::IncorrectType => write!(f, "incorrect type"),
            Error::Utf8(e) => write!(f, "{e}"),
            Error::PaddingNot0(b) => write!(f, "Unexpected non-0 padding byte `{b}`"),
            Error::UnknownFd => write!(f, "File descriptor not in the given FD index"),
            Error::MissingFramingOffset => write!(
                f,
                "Missing framing offset at the end of GVariant-encoded container"
            ),
            Error::IncompatibleFormat(sig, format) => {
                write!(f, "Type `{sig}` is not compatible with `{format}` format",)
            }
            Error::SignatureMismatch(provided, expected) => write!(
                f,
                "Signature mismatch: got `{provided}`, expected {expected}",
            ),
            Error::OutOfBounds => write!(
                f,
                // FIXME: using the `Debug` impl of `Range` because it doesn't impl `Display`.
                "Out of bounds range specified",
            ),
            Error::MaxDepthExceeded(max) => write!(f, "{max}"),
        }
    }
}

impl Clone for Error {
    fn clone(&self) -> Self {
        match self {
            Error::Message(s) => Error::Message(s.clone()),
            Error::InputOutput(e) => Error::InputOutput(e.clone()),
            Error::IncorrectType => Error::IncorrectType,
            Error::Utf8(e) => Error::Utf8(*e),
            Error::PaddingNot0(b) => Error::PaddingNot0(*b),
            Error::UnknownFd => Error::UnknownFd,
            Error::MissingFramingOffset => Error::MissingFramingOffset,
            Error::IncompatibleFormat(sig, format) => {
                Error::IncompatibleFormat(sig.clone(), *format)
            }
            Error::SignatureMismatch(provided, expected) => {
                Error::SignatureMismatch(provided.clone(), expected.clone())
            }
            Error::OutOfBounds => Error::OutOfBounds,
            Error::MaxDepthExceeded(max) => Error::MaxDepthExceeded(*max),
        }
    }
}

impl From<Infallible> for Error {
    fn from(i: Infallible) -> Self {
        match i {}
    }
}

impl de::Error for Error {
    // TODO: Add more specific error variants to Error enum above so we can implement other methods
    // here too.
    fn custom<T>(msg: T) -> Error
    where
        T: fmt::Display,
    {
        Error::Message(msg.to_string())
    }
}

impl ser::Error for Error {
    fn custom<T>(msg: T) -> Error
    where
        T: fmt::Display,
    {
        Error::Message(msg.to_string())
    }
}

impl From<io::Error> for Error {
    fn from(val: io::Error) -> Self {
        Error::InputOutput(Arc::new(val))
    }
}

/// Alias for a `Result` with the error type `zvariant::Error`.
pub type Result<T> = result::Result<T, Error>;
