use crate::io::AsyncWrite;
use futures_core::future::Future;
use futures_core::task::{Context, Poll};
use std::io;
use std::pin::Pin;

/// Future for the [`write`](super::AsyncWriteExt::write) method.
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct Write<'a, W: ?Sized> {
    writer: &'a mut W,
    buf: &'a [u8],
}

impl<W: ?Sized + Unpin> Unpin for Write<'_, W> {}

impl<'a, W: AsyncWrite + ?Sized + Unpin> Write<'a, W> {
    pub(super) fn new(writer: &'a mut W, buf: &'a [u8]) -> Self {
        Self { writer, buf }
    }
}

impl<W: AsyncWrite + ?Sized + Unpin> Future for Write<'_, W> {
    type Output = io::Result<usize>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = &mut *self;
        Pin::new(&mut this.writer).poll_write(cx, this.buf)
    }
}
