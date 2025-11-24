use crate::io::AsyncBufRead;
use crate::util::memchr;

use pin_project_lite::pin_project;
use std::future::Future;
use std::io;
use std::marker::PhantomPinned;
use std::mem;
use std::pin::Pin;
use std::task::{ready, Context, Poll};

pin_project! {
    /// Future for the [`read_until`](crate::io::AsyncBufReadExt::read_until) method.
    /// The delimiter is included in the resulting vector.
    #[derive(Debug)]
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct ReadUntil<'a, R: ?Sized> {
        reader: &'a mut R,
        delimiter: u8,
        buf: &'a mut Vec<u8>,
        // The number of bytes appended to buf. This can be less than buf.len() if
        // the buffer was not empty when the operation was started.
        read: usize,
        // Make this future `!Unpin` for compatibility with async trait methods.
        #[pin]
        _pin: PhantomPinned,
    }
}

pub(crate) fn read_until<'a, R>(
    reader: &'a mut R,
    delimiter: u8,
    buf: &'a mut Vec<u8>,
) -> ReadUntil<'a, R>
where
    R: AsyncBufRead + ?Sized + Unpin,
{
    ReadUntil {
        reader,
        delimiter,
        buf,
        read: 0,
        _pin: PhantomPinned,
    }
}

pub(super) fn read_until_internal<R: AsyncBufRead + ?Sized>(
    mut reader: Pin<&mut R>,
    cx: &mut Context<'_>,
    delimiter: u8,
    buf: &mut Vec<u8>,
    read: &mut usize,
) -> Poll<io::Result<usize>> {
    loop {
        let (done, used) = {
            let available = ready!(reader.as_mut().poll_fill_buf(cx))?;
            if let Some(i) = memchr::memchr(delimiter, available) {
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
            return Poll::Ready(Ok(mem::replace(read, 0)));
        }
    }
}

impl<R: AsyncBufRead + ?Sized + Unpin> Future for ReadUntil<'_, R> {
    type Output = io::Result<usize>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let me = self.project();
        read_until_internal(Pin::new(*me.reader), cx, *me.delimiter, me.buf, me.read)
    }
}
