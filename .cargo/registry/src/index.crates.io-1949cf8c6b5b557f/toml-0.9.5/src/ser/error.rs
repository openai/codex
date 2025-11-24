use crate::alloc_prelude::*;

/// Errors that can occur when serializing a type.
#[derive(Clone, PartialEq, Eq)]
pub struct Error {
    pub(crate) inner: ErrorInner,
}

impl Error {
    pub(crate) fn new(inner: impl core::fmt::Display) -> Self {
        Self {
            inner: ErrorInner::Custom(inner.to_string()),
        }
    }

    pub(crate) fn unsupported_type(t: Option<&'static str>) -> Self {
        Self {
            inner: ErrorInner::UnsupportedType(t),
        }
    }

    pub(crate) fn out_of_range(t: Option<&'static str>) -> Self {
        Self {
            inner: ErrorInner::OutOfRange(t),
        }
    }

    pub(crate) fn unsupported_none() -> Self {
        Self {
            inner: ErrorInner::UnsupportedNone,
        }
    }

    pub(crate) fn key_not_string() -> Self {
        Self {
            inner: ErrorInner::KeyNotString,
        }
    }

    #[cfg(feature = "display")]
    pub(crate) fn date_invalid() -> Self {
        Self {
            inner: ErrorInner::DateInvalid,
        }
    }
}

impl From<core::fmt::Error> for Error {
    fn from(_: core::fmt::Error) -> Self {
        Self::new("an error occurred when writing a value")
    }
}

impl serde::ser::Error for Error {
    fn custom<T>(msg: T) -> Self
    where
        T: core::fmt::Display,
    {
        Self::new(msg)
    }
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.inner.fmt(f)
    }
}

impl core::fmt::Debug for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.inner.fmt(f)
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Error {}
#[cfg(not(feature = "std"))]
impl serde::de::StdError for Error {}

/// Errors that can occur when deserializing a type.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub(crate) enum ErrorInner {
    /// Type could not be serialized to TOML
    UnsupportedType(Option<&'static str>),
    /// Value was out of range for the given type
    OutOfRange(Option<&'static str>),
    /// `None` could not be serialized to TOML
    UnsupportedNone,
    /// Key was not convertible to `String` for serializing to TOML
    KeyNotString,
    /// A serialized date was invalid
    DateInvalid,
    /// Other serialization error
    Custom(String),
}

impl core::fmt::Display for ErrorInner {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnsupportedType(Some(t)) => write!(formatter, "unsupported {t} type"),
            Self::UnsupportedType(None) => write!(formatter, "unsupported rust type"),
            Self::OutOfRange(Some(t)) => write!(formatter, "out-of-range value for {t} type"),
            Self::OutOfRange(None) => write!(formatter, "out-of-range value"),
            Self::UnsupportedNone => "unsupported None value".fmt(formatter),
            Self::KeyNotString => "map key was not a string".fmt(formatter),
            Self::DateInvalid => "a serialized date was invalid".fmt(formatter),
            Self::Custom(s) => s.fmt(formatter),
        }
    }
}
