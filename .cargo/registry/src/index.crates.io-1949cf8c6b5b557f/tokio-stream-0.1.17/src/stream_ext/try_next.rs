use crate::stream_ext::Next;
use crate::Stream;

use core::future::Future;
use core::marker::PhantomPinned;
use core::pin::Pin;
use core::task::{Context, Poll};
use pin_project_lite::pin_project;

pin_project! {
    /// Future for the [`try_next`](super::StreamExt::try_next) method.
    ///
    /// # Cancel safety
    ///
    /// This method is cancel safe. It only
    /// holds onto a reference to the underlying stream,
    /// so dropping it will never lose a value.
    #[derive(Debug)]
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct TryNext<'a, St: ?Sized> {
        #[pin]
        inner: Next<'a, St>,
        // Make this future `!Unpin` for compatibility with async trait methods.
        #[pin]
        _pin: PhantomPinned,
    }
}

impl<'a, St: ?Sized> TryNext<'a, St> {
    pub(super) fn new(stream: &'a mut St) -> Self {
        Self {
            inner: Next::new(stream),
            _pin: PhantomPinned,
        }
    }
}

impl<T, E, St: ?Sized + Stream<Item = Result<T, E>> + Unpin> Future for TryNext<'_, St> {
    type Output = Result<Option<T>, E>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let me = self.project();
        me.inner.poll(cx).map(Option::transpose)
    }
}
