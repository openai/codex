use futures_core::ready;
use futures_core::task::{Context, Poll};
use futures_io::{AsyncRead, IoSliceMut};
use std::fmt;
use std::io;
use std::pin::Pin;

/// Reader for the [`repeat()`] function.
#[must_use = "readers do nothing unless polled"]
pub struct Repeat {
    byte: u8,
}

/// Creates an instance of a reader that infinitely repeats one byte.
///
/// All reads from this reader will succeed by filling the specified buffer with
/// the given byte.
///
/// # Examples
///
/// ```
/// # futures::executor::block_on(async {
/// use futures::io::{self, AsyncReadExt};
///
/// let mut buffer = [0; 3];
/// let mut reader = io::repeat(0b101);
/// reader.read_exact(&mut buffer).await.unwrap();
/// assert_eq!(buffer, [0b101, 0b101, 0b101]);
/// # Ok::<(), Box<dyn std::error::Error>>(()) }).unwrap();
/// ```
pub fn repeat(byte: u8) -> Repeat {
    Repeat { byte }
}

impl AsyncRead for Repeat {
    #[inline]
    fn poll_read(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        for slot in &mut *buf {
            *slot = self.byte;
        }
        Poll::Ready(Ok(buf.len()))
    }

    #[inline]
    fn poll_read_vectored(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &mut [IoSliceMut<'_>],
    ) -> Poll<io::Result<usize>> {
        let mut nwritten = 0;
        for buf in bufs {
            nwritten += ready!(self.as_mut().poll_read(cx, buf))?;
        }
        Poll::Ready(Ok(nwritten))
    }
}

impl fmt::Debug for Repeat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad("Repeat { .. }")
    }
}
