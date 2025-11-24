use futures_core::future::Future;
use futures_core::ready;
use futures_core::task::{Context, Poll};
use futures_io::AsyncBufRead;
use std::io;
use std::pin::Pin;
use std::vec::Vec;

/// Future for the [`read_until`](super::AsyncBufReadExt::read_until) method.
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct ReadUntil<'a, R: ?Sized> {
    reader: &'a mut R,
    byte: u8,
    buf: &'a mut Vec<u8>,
    read: usize,
}

impl<R: ?Sized + Unpin> Unpin for ReadUntil<'_, R> {}

impl<'a, R: AsyncBufRead + ?Sized + Unpin> ReadUntil<'a, R> {
    pub(super) fn new(reader: &'a mut R, byte: u8, buf: &'a mut Vec<u8>) -> Self {
        Self { reader, byte, buf, read: 0 }
    }
}

pub(super) fn read_until_internal<R: AsyncBufRead + ?Sized>(
    mut reader: Pin<&mut R>,
    cx: &mut Context<'_>,
    byte: u8,
    buf: &mut Vec<u8>,
    read: &mut usize,
) -> Poll<io::Result<usize>> {
    loop {
        let (done, used) = {
            let available = ready!(reader.as_mut().poll_fill_buf(cx))?;
            if let Some(i) = memchr::memchr(byte, available) {
                buf.extend_from_slice(&available[..=i]);
                (true, i + 1)
            } else {
                buf.extend_from_slice(available);
                (false, available.len())
            }
        };
        reader.as_mut().consume(used);
        *read += used;
        if done || used == 0 {
            return Poll::Ready(Ok(*read));
        }
    }
}

impl<R: AsyncBufRead + ?Sized + Unpin> Future for ReadUntil<'_, R> {
    type Output = io::Result<usize>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let Self { reader, byte, buf, read } = &mut *self;
        read_until_internal(Pin::new(reader), cx, *byte, buf, read)
    }
}
