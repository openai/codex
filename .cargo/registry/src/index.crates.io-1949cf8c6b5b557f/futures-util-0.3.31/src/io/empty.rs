use futures_core::task::{Context, Poll};
use futures_io::{AsyncBufRead, AsyncRead};
use std::fmt;
use std::io;
use std::pin::Pin;

/// Reader for the [`empty()`] function.
#[must_use = "readers do nothing unless polled"]
pub struct Empty {
    _priv: (),
}

/// Constructs a new handle to an empty reader.
///
/// All reads from the returned reader will return `Poll::Ready(Ok(0))`.
///
/// # Examples
///
/// A slightly sad example of not reading anything into a buffer:
///
/// ```
/// # futures::executor::block_on(async {
/// use futures::io::{self, AsyncReadExt};
///
/// let mut buffer = String::new();
/// let mut reader = io::empty();
/// reader.read_to_string(&mut buffer).await?;
/// assert!(buffer.is_empty());
/// # Ok::<(), Box<dyn std::error::Error>>(()) }).unwrap();
/// ```
pub fn empty() -> Empty {
    Empty { _priv: () }
}

impl AsyncRead for Empty {
    #[inline]
    fn poll_read(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
        _: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        Poll::Ready(Ok(0))
    }
}

impl AsyncBufRead for Empty {
    #[inline]
    fn poll_fill_buf(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<&[u8]>> {
        Poll::Ready(Ok(&[]))
    }
    #[inline]
    fn consume(self: Pin<&mut Self>, _: usize) {}
}

impl fmt::Debug for Empty {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad("Empty { .. }")
    }
}
