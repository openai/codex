use std::fmt;

/// An error encountered while working with structured data.
#[derive(Debug)]
pub struct Error {
    inner: Inner,
}

#[derive(Debug)]
enum Inner {
    #[cfg(feature = "std")]
    Boxed(std_support::BoxedError),
    Msg(&'static str),
    #[cfg(feature = "value-bag")]
    Value(crate::kv::value::inner::Error),
    Fmt,
}

impl Error {
    /// Create an error from a message.
    pub fn msg(msg: &'static str) -> Self {
        Error {
            inner: Inner::Msg(msg),
        }
    }

    // Not public so we don't leak the `crate::kv::value::inner` API
    #[cfg(feature = "value-bag")]
    pub(super) fn from_value(err: crate::kv::value::inner::Error) -> Self {
        Error {
            inner: Inner::Value(err),
        }
    }

    // Not public so we don't leak the `crate::kv::value::inner` API
    #[cfg(feature = "value-bag")]
    pub(super) fn into_value(self) -> crate::kv::value::inner::Error {
        match self.inner {
            Inner::Value(err) => err,
            #[cfg(feature = "kv_std")]
            _ => crate::kv::value::inner::Error::boxed(self),
            #[cfg(not(feature = "kv_std"))]
            _ => crate::kv::value::inner::Error::msg("error inspecting a value"),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::Inner::*;
        match &self.inner {
            #[cfg(feature = "std")]
            Boxed(err) => err.fmt(f),
            #[cfg(feature = "value-bag")]
            Value(err) => err.fmt(f),
            Msg(msg) => msg.fmt(f),
            Fmt => fmt::Error.fmt(f),
        }
    }
}

impl From<fmt::Error> for Error {
    fn from(_: fmt::Error) -> Self {
        Error { inner: Inner::Fmt }
    }
}

#[cfg(feature = "std")]
mod std_support {
    use super::*;
    use std::{error, io};

    pub(super) type BoxedError = Box<dyn error::Error + Send + Sync>;

    impl Error {
        /// Create an error from a standard error type.
        pub fn boxed<E>(err: E) -> Self
        where
            E: Into<BoxedError>,
        {
            Error {
                inner: Inner::Boxed(err.into()),
            }
        }
    }

    impl error::Error for Error {}

    impl From<io::Error> for Error {
        fn from(err: io::Error) -> Self {
            Error::boxed(err)
        }
    }
}
