use crate::io::AsyncWrite;

use pin_project_lite::pin_project;
use std::future::Future;
use std::io;
use std::marker::PhantomPinned;
use std::mem;
use std::pin::Pin;
use std::task::{ready, Context, Poll};

pin_project! {
    #[derive(Debug)]
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct WriteAll<'a, W: ?Sized> {
        writer: &'a mut W,
        buf: &'a [u8],
        // Make this future `!Unpin` for compatibility with async trait methods.
        #[pin]
        _pin: PhantomPinned,
    }
}

pub(crate) fn write_all<'a, W>(writer: &'a mut W, buf: &'a [u8]) -> WriteAll<'a, W>
where
    W: AsyncWrite + Unpin + ?Sized,
{
    WriteAll {
        writer,
        buf,
        _pin: PhantomPinned,
    }
}

impl<W> Future for WriteAll<'_, W>
where
    W: AsyncWrite + Unpin + ?Sized,
{
    type Output = io::Result<()>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let me = self.project();
        while !me.buf.is_empty() {
            let n = ready!(Pin::new(&mut *me.writer).poll_write(cx, me.buf))?;
            {
                let (_, rest) = mem::take(&mut *me.buf).split_at(n);
                *me.buf = rest;
            }
            if n == 0 {
                return Poll::Ready(Err(io::ErrorKind::WriteZero.into()));
            }
        }

        Poll::Ready(Ok(()))
    }
}
