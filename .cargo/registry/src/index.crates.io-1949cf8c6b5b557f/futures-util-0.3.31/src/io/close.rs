use futures_core::future::Future;
use futures_core::task::{Context, Poll};
use futures_io::AsyncWrite;
use std::io;
use std::pin::Pin;

/// Future for the [`close`](super::AsyncWriteExt::close) method.
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct Close<'a, W: ?Sized> {
    writer: &'a mut W,
}

impl<W: ?Sized + Unpin> Unpin for Close<'_, W> {}

impl<'a, W: AsyncWrite + ?Sized + Unpin> Close<'a, W> {
    pub(super) fn new(writer: &'a mut W) -> Self {
        Self { writer }
    }
}

impl<W: AsyncWrite + ?Sized + Unpin> Future for Close<'_, W> {
    type Output = io::Result<()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut *self.writer).poll_close(cx)
    }
}
