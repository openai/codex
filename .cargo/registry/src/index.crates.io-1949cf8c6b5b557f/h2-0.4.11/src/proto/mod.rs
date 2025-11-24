mod connection;
mod error;
mod go_away;
mod peer;
mod ping_pong;
mod settings;
mod streams;

pub(crate) use self::connection::{Config, Connection};
pub use self::error::{Error, Initiator};
pub(crate) use self::peer::{Dyn as DynPeer, Peer};
pub(crate) use self::ping_pong::UserPings;
pub(crate) use self::streams::{DynStreams, OpaqueStreamRef, StreamRef, Streams};
pub(crate) use self::streams::{Open, PollReset, Prioritized};

use crate::codec::Codec;

use self::go_away::GoAway;
use self::ping_pong::PingPong;
use self::settings::Settings;

use crate::frame::{self, Frame};

use bytes::Buf;

use tokio::io::AsyncWrite;

pub type PingPayload = [u8; 8];

pub type WindowSize = u32;

// Constants
pub const MAX_WINDOW_SIZE: WindowSize = (1 << 31) - 1; // i32::MAX as u32
pub const DEFAULT_REMOTE_RESET_STREAM_MAX: usize = 20;
pub const DEFAULT_LOCAL_RESET_COUNT_MAX: usize = 1024;
pub const DEFAULT_RESET_STREAM_MAX: usize = 10;
pub const DEFAULT_RESET_STREAM_SECS: u64 = 30;
pub const DEFAULT_MAX_SEND_BUFFER_SIZE: usize = 1024 * 400;
