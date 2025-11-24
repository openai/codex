#[cfg(feature = "alloc")]
use alloc::boxed::Box;
use core::convert::Infallible;
use core::error::Error as StdError;
use core::fmt;
use core::marker::PhantomData;
#[cfg(feature = "std")]
use std::io;

/// The [`Result`](std::result::Result) type with [`Error`] as default error type
pub type Result<I, E = Error> = core::result::Result<I, E>;

/// askama's error type
///
/// Used as error value for e.g. [`Template::render()`][crate::Template::render()]
/// and custom filters.
#[non_exhaustive]
#[derive(Debug)]
pub enum Error {
    /// Generic, unspecified formatting error
    Fmt,
    /// Key not present in [`Values`][crate::Values]
    ValueMissing,
    /// Incompatible value type for key in [`Values`][crate::Values]
    ValueType,
    /// An error raised by using `?` in a template
    #[cfg(feature = "alloc")]
    Custom(Box<dyn StdError + Send + Sync>),
    /// JSON conversion error
    #[cfg(feature = "serde_json")]
    Json(serde_json::Error),
}

impl Error {
    /// Capture an [`StdError`]
    #[inline]
    #[cfg(feature = "alloc")]
    pub fn custom(err: impl Into<Box<dyn StdError + Send + Sync>>) -> Self {
        Self::Custom(err.into())
    }

    /// Convert this [`Error`] into a
    /// <code>[Box]&lt;dyn [StdError] + [Send] + [Sync]&gt;</code>
    #[cfg(feature = "alloc")]
    pub fn into_box(self) -> Box<dyn StdError + Send + Sync> {
        match self {
            Error::Fmt => fmt::Error.into(),
            Error::ValueMissing => Box::new(Error::ValueMissing),
            Error::ValueType => Box::new(Error::ValueType),
            Error::Custom(err) => err,
            #[cfg(feature = "serde_json")]
            Error::Json(err) => err.into(),
        }
    }

    /// Convert this [`Error`] into an [`io::Error`]
    ///
    /// Not this error itself, but the contained [`source`][StdError::source] is returned.
    #[cfg(feature = "std")]
    pub fn into_io_error(self) -> io::Error {
        io::Error::other(match self {
            Error::Custom(err) => match err.downcast() {
                Ok(err) => return *err,
                Err(err) => err,
            },
            err => err.into_box(),
        })
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Error::Fmt => Some(&fmt::Error),
            Error::ValueMissing => None,
            Error::ValueType => None,
            #[cfg(feature = "alloc")]
            Error::Custom(err) => Some(err.as_ref()),
            #[cfg(feature = "serde_json")]
            Error::Json(err) => Some(err),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Fmt => fmt::Error.fmt(f),
            Error::ValueMissing => f.write_str("key missing in values"),
            Error::ValueType => f.write_str("value has wrong type"),
            #[cfg(feature = "alloc")]
            Error::Custom(err) => err.fmt(f),
            #[cfg(feature = "serde_json")]
            Error::Json(err) => err.fmt(f),
        }
    }
}

impl From<Error> for fmt::Error {
    #[inline]
    fn from(_: Error) -> Self {
        Self
    }
}

#[cfg(feature = "std")]
impl From<Error> for io::Error {
    #[inline]
    fn from(err: Error) -> Self {
        err.into_io_error()
    }
}

impl From<fmt::Error> for Error {
    #[inline]
    fn from(_: fmt::Error) -> Self {
        Error::Fmt
    }
}

/// This conversion inspects the argument and chooses the best fitting [`Error`] variant
#[cfg(feature = "alloc")]
impl From<Box<dyn StdError + Send + Sync>> for Error {
    #[inline]
    fn from(err: Box<dyn StdError + Send + Sync>) -> Self {
        error_from_stderror(err, MAX_ERROR_UNWRAP_COUNT)
    }
}

/// This conversion inspects the argument and chooses the best fitting [`Error`] variant
#[cfg(feature = "std")]
impl From<io::Error> for Error {
    #[inline]
    fn from(err: io::Error) -> Self {
        error_from_io_error(err, MAX_ERROR_UNWRAP_COUNT)
    }
}

#[cfg(feature = "alloc")]
const MAX_ERROR_UNWRAP_COUNT: usize = 5;

#[cfg(feature = "alloc")]
fn error_from_stderror(err: Box<dyn StdError + Send + Sync>, unwraps: usize) -> Error {
    let Some(unwraps) = unwraps.checked_sub(1) else {
        return Error::Custom(err);
    };
    #[cfg(not(feature = "std"))]
    let _ = unwraps;
    match ErrorKind::inspect(err.as_ref()) {
        ErrorKind::Fmt => Error::Fmt,
        ErrorKind::Custom => Error::Custom(err),
        #[cfg(feature = "serde_json")]
        ErrorKind::Json => match err.downcast() {
            Ok(err) => Error::Json(*err),
            Err(_) => Error::Fmt, // unreachable
        },
        #[cfg(feature = "std")]
        ErrorKind::Io => match err.downcast() {
            Ok(err) => error_from_io_error(*err, unwraps),
            Err(_) => Error::Fmt, // unreachable
        },
        ErrorKind::Askama => match err.downcast() {
            Ok(err) => *err,
            Err(_) => Error::Fmt, // unreachable
        },
    }
}

#[cfg(feature = "std")]
fn error_from_io_error(err: io::Error, unwraps: usize) -> Error {
    let Some(inner) = err.get_ref() else {
        return Error::custom(err);
    };
    let Some(unwraps) = unwraps.checked_sub(1) else {
        return match err.into_inner() {
            Some(err) => Error::Custom(err),
            None => Error::Fmt, // unreachable
        };
    };
    match ErrorKind::inspect(inner) {
        ErrorKind::Fmt => Error::Fmt,
        ErrorKind::Askama => match err.downcast() {
            Ok(err) => err,
            Err(_) => Error::Fmt, // unreachable
        },
        #[cfg(feature = "serde_json")]
        ErrorKind::Json => match err.downcast() {
            Ok(err) => Error::Json(err),
            Err(_) => Error::Fmt, // unreachable
        },
        ErrorKind::Custom => match err.into_inner() {
            Some(err) => Error::Custom(err),
            None => Error::Fmt, // unreachable
        },
        ErrorKind::Io => match err.downcast() {
            Ok(inner) => error_from_io_error(inner, unwraps),
            Err(_) => Error::Fmt, // unreachable
        },
    }
}

#[cfg(feature = "alloc")]
enum ErrorKind {
    Fmt,
    Custom,
    #[cfg(feature = "serde_json")]
    Json,
    #[cfg(feature = "std")]
    Io,
    Askama,
}

#[cfg(feature = "alloc")]
impl ErrorKind {
    fn inspect(err: &(dyn StdError + 'static)) -> ErrorKind {
        if err.is::<fmt::Error>() {
            return ErrorKind::Fmt;
        }

        #[cfg(feature = "std")]
        if err.is::<io::Error>() {
            return ErrorKind::Io;
        }

        if err.is::<Error>() {
            return ErrorKind::Askama;
        }

        #[cfg(feature = "serde_json")]
        if err.is::<serde_json::Error>() {
            return ErrorKind::Json;
        }

        ErrorKind::Custom
    }
}

#[cfg(feature = "serde_json")]
impl From<serde_json::Error> for Error {
    #[inline]
    fn from(err: serde_json::Error) -> Self {
        Error::Json(err)
    }
}

impl From<Infallible> for Error {
    #[inline]
    fn from(value: Infallible) -> Self {
        match value {}
    }
}

#[cfg(test)]
const _: () = {
    trait AssertSendSyncStatic: Send + Sync + 'static {}
    impl AssertSendSyncStatic for Error {}
};

/// Helper trait to convert a custom `?` call into a [`crate::Result`]
pub trait ResultConverter {
    /// Okay Value type of the output
    type Value;
    /// Input type
    type Input;

    /// Consume an interior mutable `self`, and turn it into a [`crate::Result`]
    fn askama_conv_result(self, result: Self::Input) -> Result<Self::Value, Error>;
}

/// Helper marker to be used with [`ResultConverter`]
#[derive(Debug, Clone, Copy)]
pub struct ErrorMarker<T>(PhantomData<Result<T>>);

impl<T> ErrorMarker<T> {
    /// Get marker for a [`Result`] type
    #[inline]
    pub fn of(_: &T) -> Self {
        Self(PhantomData)
    }
}

#[cfg(feature = "alloc")]
impl<T, E> ResultConverter for &ErrorMarker<Result<T, E>>
where
    E: Into<Box<dyn StdError + Send + Sync>>,
{
    type Value = T;
    type Input = Result<T, E>;

    #[inline]
    fn askama_conv_result(self, result: Self::Input) -> Result<Self::Value, Error> {
        result.map_err(Error::custom)
    }
}

impl<T, E> ResultConverter for &&ErrorMarker<Result<T, E>>
where
    E: Into<Error>,
{
    type Value = T;
    type Input = Result<T, E>;

    #[inline]
    fn askama_conv_result(self, result: Self::Input) -> Result<Self::Value, Error> {
        result.map_err(Into::into)
    }
}
