use crate::hpack;

use bytes::Bytes;

use std::fmt;

/// A helper macro that unpacks a sequence of 4 bytes found in the buffer with
/// the given identifier, starting at the given offset, into the given integer
/// type. Obviously, the integer type should be able to support at least 4
/// bytes.
///
/// # Examples
///
/// ```ignore
/// # // We ignore this doctest because the macro is not exported.
/// let buf: [u8; 4] = [0, 0, 0, 1];
/// assert_eq!(1u32, unpack_octets_4!(buf, 0, u32));
/// ```
macro_rules! unpack_octets_4 {
    // TODO: Get rid of this macro
    ($buf:expr, $offset:expr, $tip:ty) => {
        (($buf[$offset + 0] as $tip) << 24)
            | (($buf[$offset + 1] as $tip) << 16)
            | (($buf[$offset + 2] as $tip) << 8)
            | (($buf[$offset + 3] as $tip) << 0)
    };
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_unpack_octets_4() {
        let buf: [u8; 4] = [0, 0, 0, 1];
        assert_eq!(1u32, unpack_octets_4!(buf, 0, u32));
    }
}

mod data;
mod go_away;
mod head;
mod headers;
mod ping;
mod priority;
mod reason;
mod reset;
mod settings;
mod stream_id;
mod util;
mod window_update;

pub use self::data::Data;
pub use self::go_away::GoAway;
pub use self::head::{Head, Kind};
pub use self::headers::{
    parse_u64, Continuation, Headers, Pseudo, PushPromise, PushPromiseHeaderError,
};
pub use self::ping::Ping;
pub use self::priority::{Priority, StreamDependency};
pub use self::reason::Reason;
pub use self::reset::Reset;
pub use self::settings::Settings;
pub use self::stream_id::{StreamId, StreamIdOverflow};
pub use self::window_update::WindowUpdate;

#[cfg(feature = "unstable")]
pub use crate::hpack::BytesStr;

// Re-export some constants

pub use self::settings::{
    DEFAULT_INITIAL_WINDOW_SIZE, DEFAULT_MAX_FRAME_SIZE, DEFAULT_SETTINGS_HEADER_TABLE_SIZE,
    MAX_MAX_FRAME_SIZE,
};

pub type FrameSize = u32;

pub const HEADER_LEN: usize = 9;

#[derive(Eq, PartialEq)]
pub enum Frame<T = Bytes> {
    Data(Data<T>),
    Headers(Headers),
    Priority(Priority),
    PushPromise(PushPromise),
    Settings(Settings),
    Ping(Ping),
    GoAway(GoAway),
    WindowUpdate(WindowUpdate),
    Reset(Reset),
}

impl<T> Frame<T> {
    pub fn map<F, U>(self, f: F) -> Frame<U>
    where
        F: FnOnce(T) -> U,
    {
        use self::Frame::*;

        match self {
            Data(frame) => frame.map(f).into(),
            Headers(frame) => frame.into(),
            Priority(frame) => frame.into(),
            PushPromise(frame) => frame.into(),
            Settings(frame) => frame.into(),
            Ping(frame) => frame.into(),
            GoAway(frame) => frame.into(),
            WindowUpdate(frame) => frame.into(),
            Reset(frame) => frame.into(),
        }
    }
}

impl<T> fmt::Debug for Frame<T> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        use self::Frame::*;

        match *self {
            Data(ref frame) => fmt::Debug::fmt(frame, fmt),
            Headers(ref frame) => fmt::Debug::fmt(frame, fmt),
            Priority(ref frame) => fmt::Debug::fmt(frame, fmt),
            PushPromise(ref frame) => fmt::Debug::fmt(frame, fmt),
            Settings(ref frame) => fmt::Debug::fmt(frame, fmt),
            Ping(ref frame) => fmt::Debug::fmt(frame, fmt),
            GoAway(ref frame) => fmt::Debug::fmt(frame, fmt),
            WindowUpdate(ref frame) => fmt::Debug::fmt(frame, fmt),
            Reset(ref frame) => fmt::Debug::fmt(frame, fmt),
        }
    }
}

/// Errors that can occur during parsing an HTTP/2 frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// A length value other than 8 was set on a PING message.
    BadFrameSize,

    /// The padding length was larger than the frame-header-specified
    /// length of the payload.
    TooMuchPadding,

    /// An invalid setting value was provided
    InvalidSettingValue,

    /// An invalid window update value
    InvalidWindowUpdateValue,

    /// The payload length specified by the frame header was not the
    /// value necessary for the specific frame type.
    InvalidPayloadLength,

    /// Received a payload with an ACK settings frame
    InvalidPayloadAckSettings,

    /// An invalid stream identifier was provided.
    ///
    /// This is returned if a SETTINGS or PING frame is received with a stream
    /// identifier other than zero.
    InvalidStreamId,

    /// A request or response is malformed.
    MalformedMessage,

    /// An invalid stream dependency ID was provided
    ///
    /// This is returned if a HEADERS or PRIORITY frame is received with an
    /// invalid stream identifier.
    InvalidDependencyId,

    /// Failed to perform HPACK decoding
    Hpack(hpack::DecoderError),
}
