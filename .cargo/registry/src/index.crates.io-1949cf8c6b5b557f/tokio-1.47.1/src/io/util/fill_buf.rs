use crate::io::AsyncBufRead;

use pin_project_lite::pin_project;
use std::future::Future;
use std::io;
use std::marker::PhantomPinned;
use std::pin::Pin;
use std::task::{Context, Poll};

pin_project! {
    /// Future for the [`fill_buf`](crate::io::AsyncBufReadExt::fill_buf) method.
    #[derive(Debug)]
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct FillBuf<'a, R: ?Sized> {
        reader: Option<&'a mut R>,
        #[pin]
        _pin: PhantomPinned,
    }
}

pub(crate) fn fill_buf<R>(reader: &mut R) -> FillBuf<'_, R>
where
    R: AsyncBufRead + ?Sized + Unpin,
{
    FillBuf {
        reader: Some(reader),
        _pin: PhantomPinned,
    }
}

impl<'a, R: AsyncBufRead + ?Sized + Unpin> Future for FillBuf<'a, R> {
    type Output = io::Result<&'a [u8]>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let me = self.project();

        let reader = me.reader.take().expect("Polled after completion.");
        match Pin::new(&mut *reader).poll_fill_buf(cx) {
            Poll::Ready(Ok(slice)) => unsafe {
                // Safety: This is necessary only due to a limitation in the
                // borrow checker. Once Rust starts using the polonius borrow
                // checker, this can be simplified.
                //
                // The safety of this transmute relies on the fact that the
                // value of `reader` is `None` when we return in this branch.
                // Otherwise the caller could poll us again after
                // completion, and access the mutable reference while the
                // returned immutable reference still exists.
                let slice = std::mem::transmute::<&[u8], &'a [u8]>(slice);
                Poll::Ready(Ok(slice))
            },
            Poll::Ready(Err(err)) => Poll::Ready(Err(err)),
            Poll::Pending => {
                *me.reader = Some(reader);
                Poll::Pending
            }
        }
    }
}
