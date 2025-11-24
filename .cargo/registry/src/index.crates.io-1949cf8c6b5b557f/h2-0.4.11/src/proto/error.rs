use crate::codec::SendError;
use crate::frame::{Reason, StreamId};

use bytes::Bytes;
use std::fmt;
use std::io;

/// Either an H2 reason  or an I/O error
#[derive(Clone, Debug)]
pub enum Error {
    Reset(StreamId, Reason, Initiator),
    GoAway(Bytes, Reason, Initiator),
    Io(io::ErrorKind, Option<String>),
}

pub struct GoAway {
    pub debug_data: Bytes,
    pub reason: Reason,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Initiator {
    User,
    Library,
    Remote,
}

impl Error {
    pub(crate) fn is_local(&self) -> bool {
        match *self {
            Self::Reset(_, _, initiator) | Self::GoAway(_, _, initiator) => initiator.is_local(),
            Self::Io(..) => true,
        }
    }

    pub(crate) fn user_go_away(reason: Reason) -> Self {
        Self::GoAway(Bytes::new(), reason, Initiator::User)
    }

    pub(crate) fn library_reset(stream_id: StreamId, reason: Reason) -> Self {
        Self::Reset(stream_id, reason, Initiator::Library)
    }

    pub(crate) fn library_go_away(reason: Reason) -> Self {
        Self::GoAway(Bytes::new(), reason, Initiator::Library)
    }

    pub(crate) fn library_go_away_data(reason: Reason, debug_data: impl Into<Bytes>) -> Self {
        Self::GoAway(debug_data.into(), reason, Initiator::Library)
    }

    pub(crate) fn remote_reset(stream_id: StreamId, reason: Reason) -> Self {
        Self::Reset(stream_id, reason, Initiator::Remote)
    }

    pub(crate) fn remote_go_away(debug_data: Bytes, reason: Reason) -> Self {
        Self::GoAway(debug_data, reason, Initiator::Remote)
    }
}

impl Initiator {
    fn is_local(&self) -> bool {
        match *self {
            Self::User | Self::Library => true,
            Self::Remote => false,
        }
    }

    pub(crate) fn is_library(&self) -> bool {
        matches!(self, Self::Library)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Self::Reset(_, reason, _) | Self::GoAway(_, reason, _) => reason.fmt(fmt),
            Self::Io(_, Some(ref inner)) => inner.fmt(fmt),
            Self::Io(kind, None) => io::Error::from(kind).fmt(fmt),
        }
    }
}

impl From<io::ErrorKind> for Error {
    fn from(src: io::ErrorKind) -> Self {
        Error::Io(src, None)
    }
}

impl From<io::Error> for Error {
    fn from(src: io::Error) -> Self {
        Error::Io(src.kind(), src.get_ref().map(|inner| inner.to_string()))
    }
}

impl From<Error> for SendError {
    fn from(src: Error) -> Self {
        Self::Connection(src)
    }
}
