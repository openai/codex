use crate::io::AsyncRead;
use futures_core::future::Future;
use futures_core::ready;
use futures_core::task::{Context, Poll};
use std::io;
use std::mem;
use std::pin::Pin;

/// Future for the [`read_exact`](super::AsyncReadExt::read_exact) method.
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct ReadExact<'a, R: ?Sized> {
    reader: &'a mut R,
    buf: &'a mut [u8],
}

impl<R: ?Sized + Unpin> Unpin for ReadExact<'_, R> {}

impl<'a, R: AsyncRead + ?Sized + Unpin> ReadExact<'a, R> {
    pub(super) fn new(reader: &'a mut R, buf: &'a mut [u8]) -> Self {
        Self { reader, buf }
    }
}

impl<R: AsyncRead + ?Sized + Unpin> Future for ReadExact<'_, R> {
    type Output = io::Result<()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = &mut *self;
        while !this.buf.is_empty() {
            let n = ready!(Pin::new(&mut this.reader).poll_read(cx, this.buf))?;
            {
                let (_, rest) = mem::take(&mut this.buf).split_at_mut(n);
                this.buf = rest;
            }
            if n == 0 {
                return Poll::Ready(Err(io::ErrorKind::UnexpectedEof.into()));
            }
        }
        Poll::Ready(Ok(()))
    }
}
