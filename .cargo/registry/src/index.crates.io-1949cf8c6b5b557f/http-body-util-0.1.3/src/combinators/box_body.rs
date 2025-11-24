use crate::BodyExt as _;

use bytes::Buf;
use http_body::{Body, Frame, SizeHint};
use std::{
    fmt,
    pin::Pin,
    task::{Context, Poll},
};

/// A boxed [`Body`] trait object.
pub struct BoxBody<D, E> {
    inner: Pin<Box<dyn Body<Data = D, Error = E> + Send + Sync + 'static>>,
}

/// A boxed [`Body`] trait object that is !Sync.
pub struct UnsyncBoxBody<D, E> {
    inner: Pin<Box<dyn Body<Data = D, Error = E> + Send + 'static>>,
}

impl<D, E> BoxBody<D, E> {
    /// Create a new `BoxBody`.
    pub fn new<B>(body: B) -> Self
    where
        B: Body<Data = D, Error = E> + Send + Sync + 'static,
        D: Buf,
    {
        Self {
            inner: Box::pin(body),
        }
    }
}

impl<D, E> fmt::Debug for BoxBody<D, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BoxBody").finish()
    }
}

impl<D, E> Body for BoxBody<D, E>
where
    D: Buf,
{
    type Data = D;
    type Error = E;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        self.inner.as_mut().poll_frame(cx)
    }

    fn is_end_stream(&self) -> bool {
        self.inner.is_end_stream()
    }

    fn size_hint(&self) -> SizeHint {
        self.inner.size_hint()
    }
}

impl<D, E> Default for BoxBody<D, E>
where
    D: Buf + 'static,
{
    fn default() -> Self {
        BoxBody::new(crate::Empty::new().map_err(|err| match err {}))
    }
}

// === UnsyncBoxBody ===
impl<D, E> UnsyncBoxBody<D, E> {
    /// Create a new `UnsyncBoxBody`.
    pub fn new<B>(body: B) -> Self
    where
        B: Body<Data = D, Error = E> + Send + 'static,
        D: Buf,
    {
        Self {
            inner: Box::pin(body),
        }
    }
}

impl<D, E> fmt::Debug for UnsyncBoxBody<D, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UnsyncBoxBody").finish()
    }
}

impl<D, E> Body for UnsyncBoxBody<D, E>
where
    D: Buf,
{
    type Data = D;
    type Error = E;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        self.inner.as_mut().poll_frame(cx)
    }

    fn is_end_stream(&self) -> bool {
        self.inner.is_end_stream()
    }

    fn size_hint(&self) -> SizeHint {
        self.inner.size_hint()
    }
}

impl<D, E> Default for UnsyncBoxBody<D, E>
where
    D: Buf + 'static,
{
    fn default() -> Self {
        UnsyncBoxBody::new(crate::Empty::new().map_err(|err| match err {}))
    }
}
