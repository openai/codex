use bytes::Buf;
use http_body::{Body, Frame, SizeHint};
use std::{
    convert::Infallible,
    fmt,
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll},
};

/// A body that is always empty.
pub struct Empty<D> {
    _marker: PhantomData<fn() -> D>,
}

impl<D> Empty<D> {
    /// Create a new `Empty`.
    pub const fn new() -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

impl<D: Buf> Body for Empty<D> {
    type Data = D;
    type Error = Infallible;

    #[inline]
    fn poll_frame(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        Poll::Ready(None)
    }

    fn is_end_stream(&self) -> bool {
        true
    }

    fn size_hint(&self) -> SizeHint {
        SizeHint::with_exact(0)
    }
}

impl<D> fmt::Debug for Empty<D> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Empty").finish()
    }
}

impl<D> Default for Empty<D> {
    fn default() -> Self {
        Self::new()
    }
}

impl<D> Clone for Empty<D> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<D> Copy for Empty<D> {}
