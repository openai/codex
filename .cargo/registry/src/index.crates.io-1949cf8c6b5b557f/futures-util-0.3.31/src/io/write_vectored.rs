use crate::io::AsyncWrite;
use futures_core::future::Future;
use futures_core::task::{Context, Poll};
use std::io::{self, IoSlice};
use std::pin::Pin;

/// Future for the [`write_vectored`](super::AsyncWriteExt::write_vectored) method.
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct WriteVectored<'a, W: ?Sized> {
    writer: &'a mut W,
    bufs: &'a [IoSlice<'a>],
}

impl<W: ?Sized + Unpin> Unpin for WriteVectored<'_, W> {}

impl<'a, W: AsyncWrite + ?Sized + Unpin> WriteVectored<'a, W> {
    pub(super) fn new(writer: &'a mut W, bufs: &'a [IoSlice<'a>]) -> Self {
        Self { writer, bufs }
    }
}

impl<W: AsyncWrite + ?Sized + Unpin> Future for WriteVectored<'_, W> {
    type Output = io::Result<usize>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = &mut *self;
        Pin::new(&mut this.writer).poll_write_vectored(cx, this.bufs)
    }
}
