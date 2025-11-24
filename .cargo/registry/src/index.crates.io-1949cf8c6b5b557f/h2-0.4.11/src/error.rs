use crate::codec::{SendError, UserError};
use crate::frame::StreamId;
use crate::proto::{self, Initiator};

use bytes::Bytes;
use std::{error, fmt, io};

pub use crate::frame::Reason;

/// Represents HTTP/2 operation errors.
///
/// `Error` covers error cases raised by protocol errors caused by the
/// peer, I/O (transport) errors, and errors caused by the user of the library.
///
/// If the error was caused by the remote peer, then it will contain a
/// [`Reason`] which can be obtained with the [`reason`] function.
///
/// [`Reason`]: struct.Reason.html
/// [`reason`]: #method.reason
#[derive(Debug)]
pub struct Error {
    kind: Kind,
}

#[derive(Debug)]
enum Kind {
    /// A RST_STREAM frame was received or sent.
    #[allow(dead_code)]
    Reset(StreamId, Reason, Initiator),

    /// A GO_AWAY frame was received or sent.
    GoAway(Bytes, Reason, Initiator),

    /// The user created an error from a bare Reason.
    Reason(Reason),

    /// An error resulting from an invalid action taken by the user of this
    /// library.
    User(UserError),

    /// An `io::Error` occurred while trying to read or write.
    Io(io::Error),
}

// ===== impl Error =====

impl Error {
    /// If the error was caused by the remote peer, the error reason.
    ///
    /// This is either an error received by the peer or caused by an invalid
    /// action taken by the peer (i.e. a protocol error).
    pub fn reason(&self) -> Option<Reason> {
        match self.kind {
            Kind::Reset(_, reason, _) | Kind::GoAway(_, reason, _) | Kind::Reason(reason) => {
                Some(reason)
            }
            _ => None,
        }
    }

    /// Returns true if the error is an io::Error
    pub fn is_io(&self) -> bool {
        matches!(self.kind, Kind::Io(..))
    }

    /// Returns the error if the error is an io::Error
    pub fn get_io(&self) -> Option<&io::Error> {
        match self.kind {
            Kind::Io(ref e) => Some(e),
            _ => None,
        }
    }

    /// Returns the error if the error is an io::Error
    pub fn into_io(self) -> Option<io::Error> {
        match self.kind {
            Kind::Io(e) => Some(e),
            _ => None,
        }
    }

    pub(crate) fn from_io(err: io::Error) -> Self {
        Error {
            kind: Kind::Io(err),
        }
    }

    /// Returns true if the error is from a `GOAWAY`.
    pub fn is_go_away(&self) -> bool {
        matches!(self.kind, Kind::GoAway(..))
    }

    /// Returns true if the error is from a `RST_STREAM`.
    pub fn is_reset(&self) -> bool {
        matches!(self.kind, Kind::Reset(..))
    }

    /// Returns true if the error was received in a frame from the remote.
    ///
    /// Such as from a received `RST_STREAM` or `GOAWAY` frame.
    pub fn is_remote(&self) -> bool {
        matches!(
            self.kind,
            Kind::GoAway(_, _, Initiator::Remote) | Kind::Reset(_, _, Initiator::Remote)
        )
    }

    /// Returns true if the error was created by `h2`.
    ///
    /// Such as noticing some protocol error and sending a GOAWAY or RST_STREAM.
    pub fn is_library(&self) -> bool {
        matches!(
            self.kind,
            Kind::GoAway(_, _, Initiator::Library) | Kind::Reset(_, _, Initiator::Library)
        )
    }
}

impl From<proto::Error> for Error {
    fn from(src: proto::Error) -> Error {
        use crate::proto::Error::*;

        Error {
            kind: match src {
                Reset(stream_id, reason, initiator) => Kind::Reset(stream_id, reason, initiator),
                GoAway(debug_data, reason, initiator) => {
                    Kind::GoAway(debug_data, reason, initiator)
                }
                Io(kind, inner) => {
                    Kind::Io(inner.map_or_else(|| kind.into(), |inner| io::Error::new(kind, inner)))
                }
            },
        }
    }
}

impl From<Reason> for Error {
    fn from(src: Reason) -> Error {
        Error {
            kind: Kind::Reason(src),
        }
    }
}

impl From<SendError> for Error {
    fn from(src: SendError) -> Error {
        match src {
            SendError::User(e) => e.into(),
            SendError::Connection(e) => e.into(),
        }
    }
}

impl From<UserError> for Error {
    fn from(src: UserError) -> Error {
        Error {
            kind: Kind::User(src),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let debug_data = match self.kind {
            Kind::Reset(_, reason, Initiator::User) => {
                return write!(fmt, "stream error sent by user: {}", reason)
            }
            Kind::Reset(_, reason, Initiator::Library) => {
                return write!(fmt, "stream error detected: {}", reason)
            }
            Kind::Reset(_, reason, Initiator::Remote) => {
                return write!(fmt, "stream error received: {}", reason)
            }
            Kind::GoAway(ref debug_data, reason, Initiator::User) => {
                write!(fmt, "connection error sent by user: {}", reason)?;
                debug_data
            }
            Kind::GoAway(ref debug_data, reason, Initiator::Library) => {
                write!(fmt, "connection error detected: {}", reason)?;
                debug_data
            }
            Kind::GoAway(ref debug_data, reason, Initiator::Remote) => {
                write!(fmt, "connection error received: {}", reason)?;
                debug_data
            }
            Kind::Reason(reason) => return write!(fmt, "protocol error: {}", reason),
            Kind::User(ref e) => return write!(fmt, "user error: {}", e),
            Kind::Io(ref e) => return e.fmt(fmt),
        };

        if !debug_data.is_empty() {
            write!(fmt, " ({:?})", debug_data)?;
        }

        Ok(())
    }
}

impl error::Error for Error {}

#[cfg(test)]
mod tests {
    use super::Error;
    use crate::Reason;

    #[test]
    fn error_from_reason() {
        let err = Error::from(Reason::HTTP_1_1_REQUIRED);
        assert_eq!(err.reason(), Some(Reason::HTTP_1_1_REQUIRED));
    }
}
