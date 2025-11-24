use pin_project_lite::pin_project;
use std::{
    pin::Pin,
    task::{Context, Poll},
};

pin_project! {
    /// Extends an underlying [`hyper`] I/O with [`tokio`] I/O implementations.
    ///
    /// This implements [`AsyncRead`] and [`AsyncWrite`] given an inner type that implements
    /// [`Read`] and [`Write`], respectively.
    #[derive(Debug)]
    pub struct WithTokioIo<I> {
        #[pin]
        inner: I,
    }
}

// ==== impl WithTokioIo =====

/// [`WithTokioIo<I>`] is [`AsyncRead`] if `I` is [`Read`].
///
/// [`AsyncRead`]: tokio::io::AsyncRead
/// [`Read`]: hyper::rt::Read
impl<I> tokio::io::AsyncRead for WithTokioIo<I>
where
    I: hyper::rt::Read,
{
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        tbuf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        //let init = tbuf.initialized().len();
        let filled = tbuf.filled().len();
        let sub_filled = unsafe {
            let mut buf = hyper::rt::ReadBuf::uninit(tbuf.unfilled_mut());

            match hyper::rt::Read::poll_read(self.project().inner, cx, buf.unfilled()) {
                Poll::Ready(Ok(())) => buf.filled().len(),
                other => return other,
            }
        };

        let n_filled = filled + sub_filled;
        // At least sub_filled bytes had to have been initialized.
        let n_init = sub_filled;
        unsafe {
            tbuf.assume_init(n_init);
            tbuf.set_filled(n_filled);
        }

        Poll::Ready(Ok(()))
    }
}

/// [`WithTokioIo<I>`] is [`AsyncWrite`] if `I` is [`Write`].
///
/// [`AsyncWrite`]: tokio::io::AsyncWrite
/// [`Write`]: hyper::rt::Write
impl<I> tokio::io::AsyncWrite for WithTokioIo<I>
where
    I: hyper::rt::Write,
{
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        hyper::rt::Write::poll_write(self.project().inner, cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), std::io::Error>> {
        hyper::rt::Write::poll_flush(self.project().inner, cx)
    }

    fn poll_shutdown(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        hyper::rt::Write::poll_shutdown(self.project().inner, cx)
    }

    fn is_write_vectored(&self) -> bool {
        hyper::rt::Write::is_write_vectored(&self.inner)
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[std::io::IoSlice<'_>],
    ) -> Poll<Result<usize, std::io::Error>> {
        hyper::rt::Write::poll_write_vectored(self.project().inner, cx, bufs)
    }
}

/// [`WithTokioIo<I>`] exposes its inner `I`'s [`Write`] implementation.
///
/// [`Write`]: hyper::rt::Write
impl<I> hyper::rt::Write for WithTokioIo<I>
where
    I: hyper::rt::Write,
{
    #[inline]
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        self.project().inner.poll_write(cx, buf)
    }

    #[inline]
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), std::io::Error>> {
        self.project().inner.poll_flush(cx)
    }

    #[inline]
    fn poll_shutdown(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        self.project().inner.poll_shutdown(cx)
    }

    #[inline]
    fn is_write_vectored(&self) -> bool {
        self.inner.is_write_vectored()
    }

    #[inline]
    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[std::io::IoSlice<'_>],
    ) -> Poll<Result<usize, std::io::Error>> {
        self.project().inner.poll_write_vectored(cx, bufs)
    }
}

impl<I> WithTokioIo<I> {
    /// Wraps the inner I/O in an [`WithTokioIo<I>`]
    pub fn new(inner: I) -> Self {
        Self { inner }
    }

    /// Returns a reference to the inner type.
    pub fn inner(&self) -> &I {
        &self.inner
    }

    /// Returns a mutable reference to the inner type.
    pub fn inner_mut(&mut self) -> &mut I {
        &mut self.inner
    }

    /// Consumes this wrapper and returns the inner type.
    pub fn into_inner(self) -> I {
        self.inner
    }
}

/// [`WithTokioIo<I>`] exposes its inner `I`'s [`Read`] implementation.
///
/// [`Read`]: hyper::rt::Read
impl<I> hyper::rt::Read for WithTokioIo<I>
where
    I: hyper::rt::Read,
{
    #[inline]
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: hyper::rt::ReadBufCursor<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        self.project().inner.poll_read(cx, buf)
    }
}
