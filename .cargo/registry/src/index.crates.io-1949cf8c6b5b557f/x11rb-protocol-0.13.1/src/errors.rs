//! This module contains the current mess that is error handling.

use crate::protocol::xproto::{SetupAuthenticate, SetupFailed};

pub use crate::id_allocator::IdsExhausted;

use core::fmt;

#[cfg(feature = "std")]
use std::error::Error;

/// An error occurred while parsing some data
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ParseError {
    /// Not enough data was provided.
    InsufficientData,

    /// A value did not fit.
    ///
    /// This error can e.g. happen when a value that was received from the X11 server does not fit
    /// into an `usize`.
    ConversionFailed,

    /// The value of an expression could not be computed.
    ///
    /// As an example, the length of the data in `xproto`'s `GetPropertyReply` is described by
    /// `value_len * (format / 8)`. The multiplication could cause an overflow, which would be
    /// represented by this error.
    InvalidExpression,

    /// A value was outside of its valid range.
    ///
    /// There are two kinds of situations where this error can happen:
    ///
    /// 1. The protocol was violated and a nonsensical value was found.
    /// 2. The user of the API called the wrong parsing function.
    ///
    /// Examples for the first kind of error:
    ///
    /// - One of a set of values should be present (a `<switch>` in xcb-proto-speak), but none of
    ///   the `<cases>` matched. This can e.g. happen when parsing
    ///   [`crate::protocol::xinput::InputInfo`].
    /// - Parsing a request with a length field that is too small for the request header to fit.
    ///
    /// Examples for the second kind of error:
    ///
    /// - Parsing an X11 error with `response_type != 0`.
    /// - Parsing an X11 reply with `response_type != 1`.
    /// - Parsing an X11 request with the wrong value for its `minor_opcode`.
    InvalidValue,

    /// Some file descriptors were expected, but not enough were received.
    MissingFileDescriptors,
}

#[cfg(feature = "std")]
impl Error for ParseError {}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::InsufficientData => write!(f, "Insufficient data was provided"),
            ParseError::ConversionFailed => {
                write!(f, "A value conversion failed due to out of range data")
            }
            ParseError::InvalidExpression => write!(
                f,
                "An expression could not be computed, e.g. due to overflow"
            ),
            ParseError::InvalidValue => {
                write!(f, "A value could not be parsed into an enumeration")
            }
            ParseError::MissingFileDescriptors => write!(f, "Missing file descriptors"),
        }
    }
}

/// An error that occurred while parsing the `$DISPLAY` environment variable
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum DisplayParsingError {
    /// No explicit display was provided and the `$DISPLAY` environment variable is not set.
    DisplayNotSet,

    /// The given value could not be parsed. The input to parsing is provided.
    MalformedValue(alloc::boxed::Box<str>),

    /// The value of `$DISPLAY` is not valid unicode
    NotUnicode,

    /// An unknown error occurred during display parsing.
    ///
    /// This is `XCB_CONN_CLOSED_PARSE_ERR`.
    Unknown,
}

#[cfg(feature = "std")]
impl Error for DisplayParsingError {}

impl fmt::Display for DisplayParsingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DisplayParsingError::DisplayNotSet => {
                write!(
                    f,
                    "$DISPLAY variable not set and no value was provided explicitly"
                )
            }
            DisplayParsingError::MalformedValue(dpy) => {
                write!(f, "Failed to parse value '{}'", dpy)
            }
            DisplayParsingError::NotUnicode => {
                write!(f, "The value of $DISPLAY is not valid unicode")
            }
            DisplayParsingError::Unknown => {
                write!(f, "Unknown error while parsing a $DISPLAY address")
            }
        }
    }
}

/// An error that occurred while connecting to an X11 server
#[derive(Debug)]
#[non_exhaustive]
pub enum ConnectError {
    /// An unknown error occurred.
    ///
    /// One situation were this error is used when libxcb indicates an error that does not match
    /// any of the defined error conditions. Thus, libxcb is violating its own API (or new error
    /// cases were defined, but are not yet handled by x11rb).
    UnknownError,

    /// Error while parsing some data, see `ParseError`.
    ParseError(ParseError),

    /// Out of memory.
    ///
    /// This is `XCB_CONN_CLOSED_MEM_INSUFFICIENT`.
    InsufficientMemory,

    /// Error during parsing of display string.
    ///
    /// This is `XCB_CONN_CLOSED_PARSE_ERR`.
    DisplayParsingError(DisplayParsingError),

    /// Server does not have a screen matching the display.
    ///
    /// This is `XCB_CONN_CLOSED_INVALID_SCREEN`.
    InvalidScreen,

    /// An I/O error occurred on the connection.
    #[cfg(feature = "std")]
    IoError(std::io::Error),

    /// Invalid ID mask provided by the server.
    ///
    /// The value of `resource_id_mask` in the `Setup` provided by the server was zero.
    ZeroIdMask,

    /// The server rejected the connection with a `SetupAuthenticate` message.
    SetupAuthenticate(SetupAuthenticate),

    /// The server rejected the connection with a `SetupFailed` message.
    SetupFailed(SetupFailed),

    /// The client did not receive enough data from the server to complete
    /// the handshake.
    Incomplete {
        /// The number of bytes that were expected.
        expected: usize,
        /// The number of bytes that were received.
        received: usize,
    },
}

#[cfg(feature = "std")]
impl Error for ConnectError {}

impl fmt::Display for ConnectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fn display(f: &mut fmt::Formatter<'_>, prefix: &str, value: &[u8]) -> fmt::Result {
            match core::str::from_utf8(value).ok() {
                Some(value) => write!(f, "{}: '{}'", prefix, value),
                None => write!(f, "{}: {:?} [message is not utf8]", prefix, value),
            }
        }
        match self {
            ConnectError::UnknownError => write!(f, "Unknown connection error"),
            ConnectError::InsufficientMemory => write!(f, "Insufficient memory"),
            ConnectError::DisplayParsingError(err) => err.fmt(f),
            ConnectError::InvalidScreen => write!(f, "Invalid screen"),
            ConnectError::ParseError(err) => err.fmt(f),
            #[cfg(feature = "std")]
            ConnectError::IoError(err) => err.fmt(f),
            ConnectError::ZeroIdMask => write!(f, "XID mask was zero"),
            ConnectError::SetupFailed(err) => display(f, "X11 setup failed", &err.reason),
            ConnectError::SetupAuthenticate(err) => {
                display(f, "X11 authentication failed", &err.reason)
            }
            ConnectError::Incomplete { expected, received } => write!(
                f,
                "Not enough data received to complete the handshake. Expected {}, received {}",
                expected, received
            ),
        }
    }
}

impl From<ParseError> for ConnectError {
    fn from(err: ParseError) -> Self {
        ConnectError::ParseError(err)
    }
}

impl From<DisplayParsingError> for ConnectError {
    fn from(err: DisplayParsingError) -> Self {
        ConnectError::DisplayParsingError(err)
    }
}

#[cfg(feature = "std")]
impl From<std::io::Error> for ConnectError {
    fn from(err: std::io::Error) -> Self {
        ConnectError::IoError(err)
    }
}
