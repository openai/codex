use crate::io::AsyncWrite;

use pin_project_lite::pin_project;
use std::io;
use std::marker::PhantomPinned;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::{future::Future, io::IoSlice};

pin_project! {
    /// A future to write a slice of buffers to an `AsyncWrite`.
    #[derive(Debug)]
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct WriteVectored<'a, 'b, W: ?Sized> {
        writer: &'a mut W,
        bufs: &'a [IoSlice<'b>],
        // Make this future `!Unpin` for compatibility with async trait methods.
        #[pin]
        _pin: PhantomPinned,
    }
}

pub(crate) fn write_vectored<'a, 'b, W>(
    writer: &'a mut W,
    bufs: &'a [IoSlice<'b>],
) -> WriteVectored<'a, 'b, W>
where
    W: AsyncWrite + Unpin + ?Sized,
{
    WriteVectored {
        writer,
        bufs,
        _pin: PhantomPinned,
    }
}

impl<W> Future for WriteVectored<'_, '_, W>
where
    W: AsyncWrite + Unpin + ?Sized,
{
    type Output = io::Result<usize>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<usize>> {
        let me = self.project();
        Pin::new(&mut *me.writer).poll_write_vectored(cx, me.bufs)
    }
}
