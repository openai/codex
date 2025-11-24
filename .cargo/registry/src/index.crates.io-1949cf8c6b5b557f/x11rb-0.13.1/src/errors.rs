//! This module contains the current mess that is error handling.

use crate::x11_utils::X11Error;

pub use x11rb_protocol::errors::{ConnectError, DisplayParsingError, IdsExhausted, ParseError};

/// An error occurred  while dynamically loading libxcb.
#[cfg(feature = "dl-libxcb")]
#[derive(Debug, Clone)]
pub enum LibxcbLoadError {
    /// Could not open the library. The `OsString` is the library
    /// file name and the string is the reason.
    OpenLibError(std::ffi::OsString, String),
    /// Could not get a symbol from the library. The byte vector is the
    /// symbol name and the string is the reason.
    GetSymbolError(Vec<u8>, String),
}

#[cfg(feature = "dl-libxcb")]
impl std::fmt::Display for LibxcbLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LibxcbLoadError::OpenLibError(lib_name, e) => {
                write!(f, "failed to open library {:?}: {}", lib_name, e)
            }
            LibxcbLoadError::GetSymbolError(symbol, e) => write!(
                f,
                "failed to get symbol \"{}\": {}",
                symbol.escape_ascii(),
                e,
            ),
        }
    }
}

#[cfg(feature = "dl-libxcb")]
impl std::error::Error for LibxcbLoadError {}

/// An error that occurred on an already established X11 connection
#[derive(Debug)]
#[non_exhaustive]
pub enum ConnectionError {
    /// An unknown error occurred.
    ///
    /// One situation were this error is used when libxcb indicates an error that does not match
    /// any of the defined error conditions. Thus, libxcb is violating its own API (or new error
    /// cases were defined, but are not yet handled by x11rb).
    UnknownError,

    /// An X11 extension was not supported by the server.
    ///
    /// This corresponds to `XCB_CONN_CLOSED_EXT_NOTSUPPORTED`.
    UnsupportedExtension,

    /// A request larger than the maximum request length was sent.
    ///
    /// This corresponds to `XCB_CONN_CLOSED_REQ_LEN_EXCEED`.
    MaximumRequestLengthExceeded,

    /// File descriptor passing failed.
    ///
    /// This corresponds to `XCB_CONN_CLOSED_FDPASSING_FAILED`.
    FdPassingFailed,

    /// Error while parsing some data, see `ParseError`.
    ParseError(ParseError),

    /// Out of memory.
    ///
    /// This is `XCB_CONN_CLOSED_MEM_INSUFFICIENT`.
    InsufficientMemory,

    /// An I/O error occurred on the connection.
    IoError(std::io::Error),
}

impl std::error::Error for ConnectionError {}

impl std::fmt::Display for ConnectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConnectionError::UnknownError => write!(f, "Unknown connection error"),
            ConnectionError::UnsupportedExtension => write!(f, "Unsupported extension"),
            ConnectionError::InsufficientMemory => write!(f, "Insufficient memory"),
            ConnectionError::MaximumRequestLengthExceeded => {
                write!(f, "Maximum request length exceeded")
            }
            ConnectionError::FdPassingFailed => write!(f, "FD passing failed"),
            ConnectionError::ParseError(err) => err.fmt(f),
            ConnectionError::IoError(err) => err.fmt(f),
        }
    }
}

impl From<ParseError> for ConnectionError {
    fn from(err: ParseError) -> Self {
        ConnectionError::ParseError(err)
    }
}

impl From<std::io::Error> for ConnectionError {
    fn from(err: std::io::Error) -> Self {
        ConnectionError::IoError(err)
    }
}

/// An error that occurred with some request.
#[derive(Debug)]
pub enum ReplyError {
    /// Some error occurred on the X11 connection.
    ConnectionError(ConnectionError),
    /// The X11 server sent an error in response to a request.
    X11Error(X11Error),
}

impl std::error::Error for ReplyError {}

impl std::fmt::Display for ReplyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReplyError::ConnectionError(e) => write!(f, "{}", e),
            ReplyError::X11Error(e) => write!(f, "X11 error {:?}", e),
        }
    }
}

impl From<ParseError> for ReplyError {
    fn from(err: ParseError) -> Self {
        Self::from(ConnectionError::from(err))
    }
}

impl From<std::io::Error> for ReplyError {
    fn from(err: std::io::Error) -> Self {
        ConnectionError::from(err).into()
    }
}

impl From<ConnectionError> for ReplyError {
    fn from(err: ConnectionError) -> Self {
        Self::ConnectionError(err)
    }
}

impl From<X11Error> for ReplyError {
    fn from(err: X11Error) -> Self {
        Self::X11Error(err)
    }
}

/// An error caused by some request or by the exhaustion of IDs.
#[derive(Debug)]
pub enum ReplyOrIdError {
    /// All available IDs have been exhausted.
    IdsExhausted,
    /// Some error occurred on the X11 connection.
    ConnectionError(ConnectionError),
    /// The X11 server sent an error in response to a request.
    X11Error(X11Error),
}

impl std::fmt::Display for ReplyOrIdError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReplyOrIdError::IdsExhausted => f.write_str("X11 IDs have been exhausted"),
            ReplyOrIdError::ConnectionError(e) => write!(f, "{}", e),
            ReplyOrIdError::X11Error(e) => write!(f, "X11 error {:?}", e),
        }
    }
}

impl std::error::Error for ReplyOrIdError {}

impl From<ParseError> for ReplyOrIdError {
    fn from(err: ParseError) -> Self {
        ConnectionError::from(err).into()
    }
}

impl From<ConnectionError> for ReplyOrIdError {
    fn from(err: ConnectionError) -> Self {
        ReplyOrIdError::ConnectionError(err)
    }
}

impl From<X11Error> for ReplyOrIdError {
    fn from(err: X11Error) -> Self {
        ReplyOrIdError::X11Error(err)
    }
}

impl From<ReplyError> for ReplyOrIdError {
    fn from(err: ReplyError) -> Self {
        match err {
            ReplyError::ConnectionError(err) => ReplyOrIdError::ConnectionError(err),
            ReplyError::X11Error(err) => ReplyOrIdError::X11Error(err),
        }
    }
}

impl From<IdsExhausted> for ReplyOrIdError {
    fn from(_: IdsExhausted) -> Self {
        ReplyOrIdError::IdsExhausted
    }
}
