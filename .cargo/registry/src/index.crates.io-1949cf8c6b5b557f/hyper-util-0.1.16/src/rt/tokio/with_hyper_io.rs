use pin_project_lite::pin_project;
use std::{
    pin::Pin,
    task::{Context, Poll},
};

pin_project! {
    /// Extends an underlying [`tokio`] I/O with [`hyper`] I/O implementations.
    ///
    /// This implements [`Read`] and [`Write`] given an inner type that implements [`AsyncRead`]
    /// and [`AsyncWrite`], respectively.
    #[derive(Debug)]
    pub struct WithHyperIo<I> {
        #[pin]
        inner: I,
    }
}

// ==== impl WithHyperIo =====

impl<I> WithHyperIo<I> {
    /// Wraps the inner I/O in an [`WithHyperIo<I>`]
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

/// [`WithHyperIo<I>`] is [`Read`] if `I` is [`AsyncRead`].
///
/// [`AsyncRead`]: tokio::io::AsyncRead
/// [`Read`]: hyper::rt::Read
impl<I> hyper::rt::Read for WithHyperIo<I>
where
    I: tokio::io::AsyncRead,
{
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        mut buf: hyper::rt::ReadBufCursor<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        let n = unsafe {
            let mut tbuf = tokio::io::ReadBuf::uninit(buf.as_mut());
            match tokio::io::AsyncRead::poll_read(self.project().inner, cx, &mut tbuf) {
                Poll::Ready(Ok(())) => tbuf.filled().len(),
                other => return other,
            }
        };

        unsafe {
            buf.advance(n);
        }
        Poll::Ready(Ok(()))
    }
}

/// [`WithHyperIo<I>`] is [`Write`] if `I` is [`AsyncWrite`].
///
/// [`AsyncWrite`]: tokio::io::AsyncWrite
/// [`Write`]: hyper::rt::Write
impl<I> hyper::rt::Write for WithHyperIo<I>
where
    I: tokio::io::AsyncWrite,
{
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        tokio::io::AsyncWrite::poll_write(self.project().inner, cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), std::io::Error>> {
        tokio::io::AsyncWrite::poll_flush(self.project().inner, cx)
    }

    fn poll_shutdown(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        tokio::io::AsyncWrite::poll_shutdown(self.project().inner, cx)
    }

    fn is_write_vectored(&self) -> bool {
        tokio::io::AsyncWrite::is_write_vectored(&self.inner)
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[std::io::IoSlice<'_>],
    ) -> Poll<Result<usize, std::io::Error>> {
        tokio::io::AsyncWrite::poll_write_vectored(self.project().inner, cx, bufs)
    }
}

/// [`WithHyperIo<I>`] exposes its inner `I`'s [`AsyncRead`] implementation.
///
/// [`AsyncRead`]: tokio::io::AsyncRead
impl<I> tokio::io::AsyncRead for WithHyperIo<I>
where
    I: tokio::io::AsyncRead,
{
    #[inline]
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        self.project().inner.poll_read(cx, buf)
    }
}

/// [`WithHyperIo<I>`] exposes its inner `I`'s [`AsyncWrite`] implementation.
///
/// [`AsyncWrite`]: tokio::io::AsyncWrite
impl<I> tokio::io::AsyncWrite for WithHyperIo<I>
where
    I: tokio::io::AsyncWrite,
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
