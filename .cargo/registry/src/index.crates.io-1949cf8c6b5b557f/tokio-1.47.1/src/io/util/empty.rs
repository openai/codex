use crate::io::util::poll_proceed_and_make_progress;
use crate::io::{AsyncBufRead, AsyncRead, AsyncSeek, AsyncWrite, ReadBuf};

use std::fmt;
use std::io::{self, SeekFrom};
use std::pin::Pin;
use std::task::{ready, Context, Poll};

cfg_io_util! {
    /// `Empty` ignores any data written via [`AsyncWrite`], and will always be empty
    /// (returning zero bytes) when read via [`AsyncRead`].
    ///
    /// This struct is generally created by calling [`empty`]. Please see
    /// the documentation of [`empty()`][`empty`] for more details.
    ///
    /// This is an asynchronous version of [`std::io::empty`][std].
    ///
    /// [`empty`]: fn@empty
    /// [std]: std::io::empty
    pub struct Empty {
        _p: (),
    }

    /// Creates a value that is always at EOF for reads, and ignores all data written.
    ///
    /// All writes on the returned instance will return `Poll::Ready(Ok(buf.len()))`
    /// and the contents of the buffer will not be inspected.
    ///
    /// All reads from the returned instance will return `Poll::Ready(Ok(0))`.
    ///
    /// This is an asynchronous version of [`std::io::empty`][std].
    ///
    /// [std]: std::io::empty
    ///
    /// # Examples
    ///
    /// A slightly sad example of not reading anything into a buffer:
    ///
    /// ```
    /// use tokio::io::{self, AsyncReadExt};
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let mut buffer = String::new();
    ///     io::empty().read_to_string(&mut buffer).await.unwrap();
    ///     assert!(buffer.is_empty());
    /// }
    /// ```
    ///
    /// A convoluted way of getting the length of a buffer:
    ///
    /// ```
    /// use tokio::io::{self, AsyncWriteExt};
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let buffer = vec![1, 2, 3, 5, 8];
    ///     let num_bytes = io::empty().write(&buffer).await.unwrap();
    ///     assert_eq!(num_bytes, 5);
    /// }
    /// ```
    pub fn empty() -> Empty {
        Empty { _p: () }
    }
}

impl AsyncRead for Empty {
    #[inline]
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        _: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        ready!(crate::trace::trace_leaf(cx));
        ready!(poll_proceed_and_make_progress(cx));
        Poll::Ready(Ok(()))
    }
}

impl AsyncBufRead for Empty {
    #[inline]
    fn poll_fill_buf(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<&[u8]>> {
        ready!(crate::trace::trace_leaf(cx));
        ready!(poll_proceed_and_make_progress(cx));
        Poll::Ready(Ok(&[]))
    }

    #[inline]
    fn consume(self: Pin<&mut Self>, _: usize) {}
}

impl AsyncWrite for Empty {
    #[inline]
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        ready!(crate::trace::trace_leaf(cx));
        ready!(poll_proceed_and_make_progress(cx));
        Poll::Ready(Ok(buf.len()))
    }

    #[inline]
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        ready!(crate::trace::trace_leaf(cx));
        ready!(poll_proceed_and_make_progress(cx));
        Poll::Ready(Ok(()))
    }

    #[inline]
    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        ready!(crate::trace::trace_leaf(cx));
        ready!(poll_proceed_and_make_progress(cx));
        Poll::Ready(Ok(()))
    }

    #[inline]
    fn is_write_vectored(&self) -> bool {
        true
    }

    #[inline]
    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[io::IoSlice<'_>],
    ) -> Poll<Result<usize, io::Error>> {
        ready!(crate::trace::trace_leaf(cx));
        ready!(poll_proceed_and_make_progress(cx));
        let num_bytes = bufs.iter().map(|b| b.len()).sum();
        Poll::Ready(Ok(num_bytes))
    }
}

impl AsyncSeek for Empty {
    #[inline]
    fn start_seek(self: Pin<&mut Self>, _position: SeekFrom) -> io::Result<()> {
        Ok(())
    }

    #[inline]
    fn poll_complete(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<u64>> {
        ready!(crate::trace::trace_leaf(cx));
        ready!(poll_proceed_and_make_progress(cx));
        Poll::Ready(Ok(0))
    }
}

impl fmt::Debug for Empty {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad("Empty { .. }")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assert_unpin() {
        crate::is_unpin::<Empty>();
    }
}
