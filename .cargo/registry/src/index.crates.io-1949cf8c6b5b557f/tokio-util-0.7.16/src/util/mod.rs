mod maybe_dangling;
#[cfg(any(feature = "io", feature = "codec"))]
mod poll_buf;

pub(crate) use maybe_dangling::MaybeDangling;
#[cfg(any(feature = "io", feature = "codec"))]
#[cfg_attr(not(feature = "io"), allow(unreachable_pub))]
pub use poll_buf::{poll_read_buf, poll_write_buf};
