mod buffer;
mod counts;
mod flow_control;
mod prioritize;
mod recv;
mod send;
mod state;
mod store;
mod stream;
#[allow(clippy::module_inception)]
mod streams;

pub(crate) use self::prioritize::Prioritized;
pub(crate) use self::recv::Open;
pub(crate) use self::send::PollReset;
pub(crate) use self::streams::{DynStreams, OpaqueStreamRef, StreamRef, Streams};

use self::buffer::Buffer;
use self::counts::Counts;
use self::flow_control::FlowControl;
use self::prioritize::Prioritize;
use self::recv::Recv;
use self::send::Send;
use self::state::State;
use self::store::Store;
use self::stream::Stream;

use crate::frame::{StreamId, StreamIdOverflow};
use crate::proto::*;

use bytes::Bytes;
use std::time::Duration;

#[derive(Debug)]
pub struct Config {
    /// Initial maximum number of locally initiated streams.
    /// After receiving a Settings frame from the remote peer,
    /// the connection will overwrite this value with the
    /// MAX_CONCURRENT_STREAMS specified in the frame.
    pub initial_max_send_streams: usize,

    /// Max amount of DATA bytes to buffer per stream.
    pub local_max_buffer_size: usize,

    /// The stream ID to start the next local stream with
    pub local_next_stream_id: StreamId,

    /// If the local peer is willing to receive push promises
    pub local_push_enabled: bool,

    /// If extended connect protocol is enabled.
    pub extended_connect_protocol_enabled: bool,

    /// How long a locally reset stream should ignore frames
    pub local_reset_duration: Duration,

    /// Maximum number of locally reset streams to keep at a time
    pub local_reset_max: usize,

    /// Maximum number of remotely reset "pending accept" streams to keep at a
    /// time. Going over this number results in a connection error.
    pub remote_reset_max: usize,

    /// Initial window size of remote initiated streams
    pub remote_init_window_sz: WindowSize,

    /// Maximum number of remote initiated streams
    pub remote_max_initiated: Option<usize>,

    /// Maximum number of locally reset streams due to protocol error across
    /// the lifetime of the connection.
    ///
    /// When this gets exceeded, we issue GOAWAYs.
    pub local_max_error_reset_streams: Option<usize>,
}
