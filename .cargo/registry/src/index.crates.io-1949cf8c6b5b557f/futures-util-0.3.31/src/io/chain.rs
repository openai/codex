use futures_core::ready;
use futures_core::task::{Context, Poll};
use futures_io::{AsyncBufRead, AsyncRead, IoSliceMut};
use pin_project_lite::pin_project;
use std::fmt;
use std::io;
use std::pin::Pin;

pin_project! {
    /// Reader for the [`chain`](super::AsyncReadExt::chain) method.
    #[must_use = "readers do nothing unless polled"]
    pub struct Chain<T, U> {
        #[pin]
        first: T,
        #[pin]
        second: U,
        done_first: bool,
    }
}

impl<T, U> Chain<T, U>
where
    T: AsyncRead,
    U: AsyncRead,
{
    pub(super) fn new(first: T, second: U) -> Self {
        Self { first, second, done_first: false }
    }

    /// Gets references to the underlying readers in this `Chain`.
    pub fn get_ref(&self) -> (&T, &U) {
        (&self.first, &self.second)
    }

    /// Gets mutable references to the underlying readers in this `Chain`.
    ///
    /// Care should be taken to avoid modifying the internal I/O state of the
    /// underlying readers as doing so may corrupt the internal state of this
    /// `Chain`.
    pub fn get_mut(&mut self) -> (&mut T, &mut U) {
        (&mut self.first, &mut self.second)
    }

    /// Gets pinned mutable references to the underlying readers in this `Chain`.
    ///
    /// Care should be taken to avoid modifying the internal I/O state of the
    /// underlying readers as doing so may corrupt the internal state of this
    /// `Chain`.
    pub fn get_pin_mut(self: Pin<&mut Self>) -> (Pin<&mut T>, Pin<&mut U>) {
        let this = self.project();
        (this.first, this.second)
    }

    /// Consumes the `Chain`, returning the wrapped readers.
    pub fn into_inner(self) -> (T, U) {
        (self.first, self.second)
    }
}

impl<T, U> fmt::Debug for Chain<T, U>
where
    T: fmt::Debug,
    U: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Chain")
            .field("t", &self.first)
            .field("u", &self.second)
            .field("done_first", &self.done_first)
            .finish()
    }
}

impl<T, U> AsyncRead for Chain<T, U>
where
    T: AsyncRead,
    U: AsyncRead,
{
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        let this = self.project();

        if !*this.done_first {
            match ready!(this.first.poll_read(cx, buf)?) {
                0 if !buf.is_empty() => *this.done_first = true,
                n => return Poll::Ready(Ok(n)),
            }
        }
        this.second.poll_read(cx, buf)
    }

    fn poll_read_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &mut [IoSliceMut<'_>],
    ) -> Poll<io::Result<usize>> {
        let this = self.project();

        if !*this.done_first {
            let n = ready!(this.first.poll_read_vectored(cx, bufs)?);
            if n == 0 && bufs.iter().any(|b| !b.is_empty()) {
                *this.done_first = true
            } else {
                return Poll::Ready(Ok(n));
            }
        }
        this.second.poll_read_vectored(cx, bufs)
    }
}

impl<T, U> AsyncBufRead for Chain<T, U>
where
    T: AsyncBufRead,
    U: AsyncBufRead,
{
    fn poll_fill_buf(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<&[u8]>> {
        let this = self.project();

        if !*this.done_first {
            match ready!(this.first.poll_fill_buf(cx)?) {
                buf if buf.is_empty() => {
                    *this.done_first = true;
                }
                buf => return Poll::Ready(Ok(buf)),
            }
        }
        this.second.poll_fill_buf(cx)
    }

    fn consume(self: Pin<&mut Self>, amt: usize) {
        let this = self.project();

        if !*this.done_first {
            this.first.consume(amt)
        } else {
            this.second.consume(amt)
        }
    }
}
