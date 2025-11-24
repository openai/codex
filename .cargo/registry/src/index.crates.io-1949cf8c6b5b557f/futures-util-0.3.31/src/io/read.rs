use crate::io::AsyncRead;
use futures_core::future::Future;
use futures_core::task::{Context, Poll};
use std::io;
use std::pin::Pin;

/// Future for the [`read`](super::AsyncReadExt::read) method.
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct Read<'a, R: ?Sized> {
    reader: &'a mut R,
    buf: &'a mut [u8],
}

impl<R: ?Sized + Unpin> Unpin for Read<'_, R> {}

impl<'a, R: AsyncRead + ?Sized + Unpin> Read<'a, R> {
    pub(super) fn new(reader: &'a mut R, buf: &'a mut [u8]) -> Self {
        Self { reader, buf }
    }
}

impl<R: AsyncRead + ?Sized + Unpin> Future for Read<'_, R> {
    type Output = io::Result<usize>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = &mut *self;
        Pin::new(&mut this.reader).poll_read(cx, this.buf)
    }
}
