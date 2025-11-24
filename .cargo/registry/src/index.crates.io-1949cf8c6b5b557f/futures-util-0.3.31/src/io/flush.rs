use futures_core::future::Future;
use futures_core::task::{Context, Poll};
use futures_io::AsyncWrite;
use std::io;
use std::pin::Pin;

/// Future for the [`flush`](super::AsyncWriteExt::flush) method.
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct Flush<'a, W: ?Sized> {
    writer: &'a mut W,
}

impl<W: ?Sized + Unpin> Unpin for Flush<'_, W> {}

impl<'a, W: AsyncWrite + ?Sized + Unpin> Flush<'a, W> {
    pub(super) fn new(writer: &'a mut W) -> Self {
        Self { writer }
    }
}

impl<W> Future for Flush<'_, W>
where
    W: AsyncWrite + ?Sized + Unpin,
{
    type Output = io::Result<()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut *self.writer).poll_flush(cx)
    }
}
