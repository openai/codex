use crate::io::AsyncWrite;

use pin_project_lite::pin_project;
use std::future::Future;
use std::io;
use std::marker::PhantomPinned;
use std::pin::Pin;
use std::task::{Context, Poll};

pin_project! {
    /// A future used to fully flush an I/O object.
    ///
    /// Created by the [`AsyncWriteExt::flush`][flush] function.
    ///
    /// [flush]: crate::io::AsyncWriteExt::flush
    #[derive(Debug)]
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct Flush<'a, A: ?Sized> {
        a: &'a mut A,
        // Make this future `!Unpin` for compatibility with async trait methods.
        #[pin]
        _pin: PhantomPinned,
    }
}

/// Creates a future which will entirely flush an I/O object.
pub(super) fn flush<A>(a: &mut A) -> Flush<'_, A>
where
    A: AsyncWrite + Unpin + ?Sized,
{
    Flush {
        a,
        _pin: PhantomPinned,
    }
}

impl<A> Future for Flush<'_, A>
where
    A: AsyncWrite + Unpin + ?Sized,
{
    type Output = io::Result<()>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let me = self.project();
        Pin::new(&mut *me.a).poll_flush(cx)
    }
}
