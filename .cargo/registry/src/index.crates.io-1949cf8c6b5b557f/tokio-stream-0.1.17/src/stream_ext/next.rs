use crate::Stream;

use core::future::Future;
use core::marker::PhantomPinned;
use core::pin::Pin;
use core::task::{Context, Poll};
use pin_project_lite::pin_project;

pin_project! {
    /// Future for the [`next`](super::StreamExt::next) method.
    ///
    /// # Cancel safety
    ///
    /// This method is cancel safe. It only
    /// holds onto a reference to the underlying stream,
    /// so dropping it will never lose a value.
    ///
    #[derive(Debug)]
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct Next<'a, St: ?Sized> {
        stream: &'a mut St,
        // Make this future `!Unpin` for compatibility with async trait methods.
        #[pin]
        _pin: PhantomPinned,
    }
}

impl<'a, St: ?Sized> Next<'a, St> {
    pub(super) fn new(stream: &'a mut St) -> Self {
        Next {
            stream,
            _pin: PhantomPinned,
        }
    }
}

impl<St: ?Sized + Stream + Unpin> Future for Next<'_, St> {
    type Output = Option<St::Item>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let me = self.project();
        Pin::new(me.stream).poll_next(cx)
    }
}
