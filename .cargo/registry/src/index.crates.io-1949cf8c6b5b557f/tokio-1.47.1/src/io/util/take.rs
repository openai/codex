use crate::io::{AsyncBufRead, AsyncRead, ReadBuf};

use pin_project_lite::pin_project;
use std::convert::TryFrom;
use std::pin::Pin;
use std::task::{ready, Context, Poll};
use std::{cmp, io};

pin_project! {
    /// Stream for the [`take`](super::AsyncReadExt::take) method.
    #[derive(Debug)]
    #[must_use = "streams do nothing unless you `.await` or poll them"]
    #[cfg_attr(docsrs, doc(cfg(feature = "io-util")))]
    pub struct Take<R> {
        #[pin]
        inner: R,
        // Add '_' to avoid conflicts with `limit` method.
        limit_: u64,
    }
}

pub(super) fn take<R: AsyncRead>(inner: R, limit: u64) -> Take<R> {
    Take {
        inner,
        limit_: limit,
    }
}

impl<R: AsyncRead> Take<R> {
    /// Returns the remaining number of bytes that can be
    /// read before this instance will return EOF.
    ///
    /// # Note
    ///
    /// This instance may reach `EOF` after reading fewer bytes than indicated by
    /// this method if the underlying [`AsyncRead`] instance reaches EOF.
    pub fn limit(&self) -> u64 {
        self.limit_
    }

    /// Sets the number of bytes that can be read before this instance will
    /// return EOF. This is the same as constructing a new `Take` instance, so
    /// the amount of bytes read and the previous limit value don't matter when
    /// calling this method.
    pub fn set_limit(&mut self, limit: u64) {
        self.limit_ = limit;
    }

    /// Gets a reference to the underlying reader.
    pub fn get_ref(&self) -> &R {
        &self.inner
    }

    /// Gets a mutable reference to the underlying reader.
    ///
    /// Care should be taken to avoid modifying the internal I/O state of the
    /// underlying reader as doing so may corrupt the internal limit of this
    /// `Take`.
    pub fn get_mut(&mut self) -> &mut R {
        &mut self.inner
    }

    /// Gets a pinned mutable reference to the underlying reader.
    ///
    /// Care should be taken to avoid modifying the internal I/O state of the
    /// underlying reader as doing so may corrupt the internal limit of this
    /// `Take`.
    pub fn get_pin_mut(self: Pin<&mut Self>) -> Pin<&mut R> {
        self.project().inner
    }

    /// Consumes the `Take`, returning the wrapped reader.
    pub fn into_inner(self) -> R {
        self.inner
    }
}

impl<R: AsyncRead> AsyncRead for Take<R> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<Result<(), io::Error>> {
        if self.limit_ == 0 {
            return Poll::Ready(Ok(()));
        }

        let me = self.project();
        let mut b = buf.take(usize::try_from(*me.limit_).unwrap_or(usize::MAX));

        let buf_ptr = b.filled().as_ptr();
        ready!(me.inner.poll_read(cx, &mut b))?;
        assert_eq!(b.filled().as_ptr(), buf_ptr);

        let n = b.filled().len();

        // We need to update the original ReadBuf
        unsafe {
            buf.assume_init(n);
        }
        buf.advance(n);
        *me.limit_ -= n as u64;
        Poll::Ready(Ok(()))
    }
}

impl<R: AsyncBufRead> AsyncBufRead for Take<R> {
    fn poll_fill_buf(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<&[u8]>> {
        let me = self.project();

        // Don't call into inner reader at all at EOF because it may still block
        if *me.limit_ == 0 {
            return Poll::Ready(Ok(&[]));
        }

        let buf = ready!(me.inner.poll_fill_buf(cx)?);
        let cap = cmp::min(buf.len() as u64, *me.limit_) as usize;
        Poll::Ready(Ok(&buf[..cap]))
    }

    fn consume(self: Pin<&mut Self>, amt: usize) {
        let me = self.project();
        // Don't let callers reset the limit by passing an overlarge value
        let amt = cmp::min(amt as u64, *me.limit_) as usize;
        *me.limit_ -= amt as u64;
        me.inner.consume(amt);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assert_unpin() {
        crate::is_unpin::<Take<()>>();
    }
}
