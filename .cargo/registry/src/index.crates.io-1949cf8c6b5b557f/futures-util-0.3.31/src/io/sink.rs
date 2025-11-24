use futures_core::task::{Context, Poll};
use futures_io::{AsyncWrite, IoSlice};
use std::fmt;
use std::io;
use std::pin::Pin;

/// Writer for the [`sink()`] function.
#[must_use = "writers do nothing unless polled"]
pub struct Sink {
    _priv: (),
}

/// Creates an instance of a writer which will successfully consume all data.
///
/// All calls to `poll_write` on the returned instance will return `Poll::Ready(Ok(buf.len()))`
/// and the contents of the buffer will not be inspected.
///
/// # Examples
///
/// ```rust
/// # futures::executor::block_on(async {
/// use futures::io::{self, AsyncWriteExt};
///
/// let buffer = vec![1, 2, 3, 5, 8];
/// let mut writer = io::sink();
/// let num_bytes = writer.write(&buffer).await?;
/// assert_eq!(num_bytes, 5);
/// # Ok::<(), Box<dyn std::error::Error>>(()) }).unwrap();
/// ```
pub fn sink() -> Sink {
    Sink { _priv: () }
}

impl AsyncWrite for Sink {
    #[inline]
    fn poll_write(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Poll::Ready(Ok(buf.len()))
    }

    #[inline]
    fn poll_write_vectored(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
        bufs: &[IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        Poll::Ready(Ok(bufs.iter().map(|b| b.len()).sum()))
    }

    #[inline]
    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
    #[inline]
    fn poll_close(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

impl fmt::Debug for Sink {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad("Sink { .. }")
    }
}
