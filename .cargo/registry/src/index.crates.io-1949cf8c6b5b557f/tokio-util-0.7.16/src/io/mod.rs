//! Helpers for IO related tasks.
//!
//! The stream types are often used in combination with hyper or reqwest, as they
//! allow converting between a hyper [`Body`] and [`AsyncRead`].
//!
//! The [`SyncIoBridge`] type converts from the world of async I/O
//! to synchronous I/O; this may often come up when using synchronous APIs
//! inside [`tokio::task::spawn_blocking`].
//!
//! [`Body`]: https://docs.rs/hyper/0.13/hyper/struct.Body.html
//! [`AsyncRead`]: tokio::io::AsyncRead

mod copy_to_bytes;
mod inspect;
mod read_buf;
mod reader_stream;
mod sink_writer;
mod stream_reader;

cfg_io_util! {
    mod read_arc;
    pub use self::read_arc::read_exact_arc;

    mod sync_bridge;
    pub use self::sync_bridge::SyncIoBridge;
}

pub use self::copy_to_bytes::CopyToBytes;
pub use self::inspect::{InspectReader, InspectWriter};
pub use self::read_buf::read_buf;
pub use self::reader_stream::ReaderStream;
pub use self::sink_writer::SinkWriter;
pub use self::stream_reader::StreamReader;
pub use crate::util::{poll_read_buf, poll_write_buf};
